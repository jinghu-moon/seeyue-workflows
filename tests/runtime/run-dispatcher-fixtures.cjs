#!/usr/bin/env node
"use strict";

// P1-N4: Host Dispatcher fixtures
// Tests for dispatchInteraction and findSyInteractBinary

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-dispatcher-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── RED: require dispatcher module ──────────────────────────────────────────
const {
  dispatchInteraction,
  findSyInteractBinary,
} = require("../../scripts/runtime/interaction-dispatcher.cjs");

const {
  ensureInteractionLayout,
  writeRequest,
} = require("../../scripts/runtime/interaction-store.cjs");

const cases = {};

// ─── CASE: findSyInteractBinary returns string or null ────────────────────────
cases["dispatcher-find-binary"] = function () {
  const result = findSyInteractBinary();
  // Result must be null (not found) or a string path
  assert(
    result === null || typeof result === "string",
    `findSyInteractBinary must return null or string, got ${typeof result}`,
  );
};

// ─── CASE: dispatchInteraction returns process error when binary not found ────
cases["dispatcher-no-binary-error"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);

  const req = {
    schema: 1,
    interaction_id: "ix-20260318-099",
    kind: "approval_request",
    status: "pending",
    title: "Test",
    message: "Test",
    selection_mode: "boolean",
    options: [
      { id: "approve", label: "批准", recommended: true },
      { id: "reject", label: "拒绝", recommended: false },
    ],
    comment_mode: "disabled",
    presentation: { mode: "text_menu", color_profile: "auto", theme: "auto" },
    originating_request_id: "req-test-099",
    created_at: new Date().toISOString(),
  };
  writeRequest(root, req);

  // Pass a non-existent binary path to force process error
  const result = dispatchInteraction(root, "ix-20260318-099", {
    binaryPath: "/nonexistent/sy-interact",
  });

  assert(typeof result === "object" && result !== null, "result must be object");
  assert(result.exitCode === 1, `exitCode must be 1 (process error), got ${result.exitCode}`);
  assert(result.error !== undefined, "result must have error field");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: dispatchInteraction returns error for missing request ───────────────
cases["dispatcher-missing-request"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);

  const result = dispatchInteraction(root, "ix-20260318-nonexistent", {
    binaryPath: "/nonexistent/sy-interact",
  });

  assert(typeof result === "object" && result !== null, "result must be object");
  assert(result.exitCode === 1, `exitCode must be 1 for missing request, got ${result.exitCode}`);
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: dispatchInteraction with mock binary that exits 2 (cancel) ─────────
cases["dispatcher-cancel-exit-code"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);

  const req = {
    schema: 1,
    interaction_id: "ix-20260318-088",
    kind: "approval_request",
    status: "pending",
    title: "Cancel Test",
    message: "Test cancel path",
    selection_mode: "boolean",
    options: [
      { id: "approve", label: "批准", recommended: true },
      { id: "reject", label: "拒绝", recommended: false },
    ],
    comment_mode: "disabled",
    presentation: { mode: "text_menu", color_profile: "auto", theme: "auto" },
    originating_request_id: "req-test-088",
    created_at: new Date().toISOString(),
  };
  writeRequest(root, req);

  // Use node as a mock binary that exits with code 2
  const nodePath = process.execPath;
  const result = dispatchInteraction(root, "ix-20260318-088", {
    binaryPath: nodePath,
    // Pass --eval to simulate exit code 2
    extraArgs: ["--eval", "process.exit(2)"],
  });

  assert(typeof result === "object" && result !== null, "result must be object");
  assert(result.exitCode === 2, `exitCode must be 2 (cancel), got ${result.exitCode}`);
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── Runner ──────────────────────────────────────────────────────────────────
function parseArgs(argv) {
  const parsed = { caseName: null };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--case") {
      index += 1;
      parsed.caseName = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${argv[index]}`);
  }
  return parsed;
}

function main() {
  const parsed = parseArgs(process.argv.slice(2));
  const selected = parsed.caseName ? [[parsed.caseName, cases[parsed.caseName]]] : Object.entries(cases);
  if (selected.some(([, run]) => typeof run !== "function")) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }
  let failed = false;
  for (const [caseName, run] of selected) {
    try {
      run();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
    }
  }
  if (failed) {
    process.exit(1);
  }
  console.log("DISPATCHER_FIXTURES_PASS");
}

main();
