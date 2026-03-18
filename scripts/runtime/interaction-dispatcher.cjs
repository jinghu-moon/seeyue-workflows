"use strict";

// interaction-dispatcher.cjs — P1-N4: Host Dispatcher
//
// Discovers the sy-interact binary, probes terminal capabilities,
// spawns sy-interact as a child process, and collects the response.
//
// This module is PRESENTER-SIDE ONLY — no policy decisions here.
// It simply launches the presenter and returns the exit code + response.

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const { readRequest, readResponse } = require("./interaction-store.cjs");

// ─── Binary discovery ────────────────────────────────────────────────────────

// Candidate paths relative to repository root, in priority order.
// The repository root is resolved from this file's location:
//   scripts/runtime/interaction-dispatcher.cjs  ->  ../../ = repo root
const REPO_ROOT = path.resolve(__dirname, "..", "..");

const CANDIDATE_PATHS = [
  path.join(REPO_ROOT, "sy-interact", "target", "debug", "sy-interact.exe"),
  path.join(REPO_ROOT, "sy-interact", "target", "debug", "sy-interact"),
  path.join(REPO_ROOT, "sy-interact", "target", "release", "sy-interact.exe"),
  path.join(REPO_ROOT, "sy-interact", "target", "release", "sy-interact"),
];

/**
 * Locate the sy-interact binary.
 * Checks candidate paths and then falls back to PATH via `which`.
 * Returns the absolute path string, or null if not found.
 */
function findSyInteractBinary() {
  // Check known build output locations first
  for (const candidate of CANDIDATE_PATHS) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }

  // Fall back to PATH lookup
  const isWindows = process.platform === "win32";
  const whichCmd = isWindows ? "where" : "which";
  const whichResult = spawnSync(whichCmd, ["sy-interact"], { encoding: "utf8" });
  if (whichResult.status === 0) {
    const found = whichResult.stdout.trim().split(/\r?\n/)[0];
    if (found && fs.existsSync(found)) {
      return found;
    }
  }

  return null;
}

// ─── Terminal capability probe ────────────────────────────────────────────────

/**
 * Run `sy-interact probe-terminal --format json` to get terminal capabilities.
 * Returns parsed JSON object, or a fallback plain object if probe fails.
 *
 * @param {string} binaryPath
 * @returns {object}
 */
function probeTerminal(binaryPath) {
  try {
    const result = spawnSync(binaryPath, ["probe-terminal", "--format", "json"], {
      encoding: "utf8",
      timeout: 5000,
    });
    if (result.status === 0 && result.stdout) {
      return JSON.parse(result.stdout.trim());
    }
  } catch {
    // fall through to default
  }
  return {
    is_tty: false,
    supports_raw_mode: false,
    supports_color: false,
    color_depth: "mono",
    terminal_kind: "unknown",
  };
}

// ─── Mode selection ───────────────────────────────────────────────────────────

/**
 * Select the presentation mode based on terminal capabilities.
 * P1 uses text_menu (not full TUI) even when is_tty=true.
 * Full TUI (tui_menu) requires is_tty=true AND supports_raw_mode=true.
 *
 * @param {object} caps — terminal capabilities from probe
 * @returns {string} "tui"|"text"|"plain"
 */
function selectMode(caps) {
  if (caps.is_tty && caps.supports_raw_mode) {
    return "tui";
  }
  if (caps.is_tty) {
    return "text";
  }
  return "plain";
}

// ─── Main dispatch ────────────────────────────────────────────────────────────

/**
 * Dispatch an interaction: read request, spawn sy-interact, read response.
 *
 * @param {string} rootDir        — workspace root
 * @param {string} interactionId  — e.g. "ix-20260318-001"
 * @param {object} opts
 *   opts.binaryPath   {string?}   override binary path (for testing)
 *   opts.extraArgs    {string[]?} extra CLI args injected after the fixed args (for testing)
 *   opts.mode         {string?}   override mode: auto|tui|text|plain
 *   opts.timeout      {number?}   timeout in seconds (0 = none, default: 120)
 * @returns {{ exitCode: number, response: object|null, error: string|null }}
 */
function dispatchInteraction(rootDir, interactionId, opts) {
  opts = opts || {};

  // 1. Read request from store
  const requestObj = readRequest(rootDir, interactionId);
  if (!requestObj) {
    return {
      exitCode: 1,
      response: null,
      error: `Interaction request not found: ${interactionId}`,
    };
  }

  // 2. Locate binary
  const binaryPath = opts.binaryPath !== undefined ? opts.binaryPath : findSyInteractBinary();
  if (!binaryPath || !fs.existsSync(binaryPath)) {
    return {
      exitCode: 1,
      response: null,
      error: `sy-interact binary not found: ${binaryPath || "(none)"}`,
    };
  }

  // 3. Determine mode
  let mode = opts.mode || "auto";
  if (mode === "auto") {
    const caps = probeTerminal(binaryPath);
    mode = selectMode(caps);
  }

  // 4. Build request + response file paths
  const interactionsBase = path.join(
    path.resolve(rootDir),
    ".ai",
    "workflow",
    "interactions",
  );
  const requestFile = path.join(interactionsBase, "requests", `${interactionId}.json`);
  const responseFile = path.join(interactionsBase, "responses", `${interactionId}.json`);

  // 5. Build args
  const timeoutSecs = opts.timeout !== undefined ? opts.timeout : 120;
  const args = [
    "render",
    "--request-file", requestFile,
    "--response-file", responseFile,
    "--mode", mode,
  ];
  if (timeoutSecs > 0) {
    args.push("--timeout", String(timeoutSecs));
  }

  // Inject extra args for testing (e.g. node --eval 'process.exit(2)')
  const extraArgs = opts.extraArgs || [];

  // 6. Spawn sy-interact synchronously
  // In production this blocks the calling JS process until the user responds.
  // The caller (controller) is responsible for running this on the appropriate thread.
  let spawnArgs;
  let spawnBin;
  if (extraArgs.length > 0) {
    // Testing mode: binaryPath IS the interpreter, extraArgs contains the script
    spawnBin = binaryPath;
    spawnArgs = [...extraArgs];
  } else {
    spawnBin = binaryPath;
    spawnArgs = args;
  }

  let spawnResult;
  try {
    spawnResult = spawnSync(spawnBin, spawnArgs, {
      stdio: ["inherit", "inherit", "inherit"],
      encoding: "utf8",
      timeout: timeoutSecs > 0 ? (timeoutSecs + 10) * 1000 : 300_000,
    });
  } catch (err) {
    return {
      exitCode: 1,
      response: null,
      error: `Failed to spawn sy-interact: ${err.message}`,
    };
  }

  const exitCode = spawnResult.status !== null ? spawnResult.status : 1;

  // Handle ETIMEDOUT from spawnSync
  if (spawnResult.error) {
    if (spawnResult.error.code === "ETIMEDOUT") {
      return { exitCode: 3, response: null, error: "sy-interact timed out" };
    }
    return { exitCode: 1, response: null, error: spawnResult.error.message };
  }

  // 7. Read response (may not exist on cancel/error)
  const response = readResponse(rootDir, interactionId);

  return { exitCode, response, error: null };
}

module.exports = {
  dispatchInteraction,
  findSyInteractBinary,
  probeTerminal,
  selectMode,
};
