"use strict";

// interaction-dispatch.cjs — P2-N5: Elicitation-first orchestration
//
// Orchestrates interaction resolution by selecting the appropriate delivery
// strategy based on client capability:
//
//   Priority: elicitation > local_presenter > text_fallback
//
// This module is the single entry point for all interaction dispatch.
// It reads the capability context, selects a strategy, delegates to the
// appropriate handler, and returns a unified DispatchResult.

const fs   = require("node:fs");
const path = require("node:path");

const { readRequest } = require("./interaction-store.cjs");
const { dispatchInteraction } = require("./interaction-dispatcher.cjs");

// ─── Strategy selection ───────────────────────────────────────────────────────

/**
 * Determine the preferred interaction strategy for the current environment.
 *
 * @param {string} rootDir  — workspace root
 * @param {object} opts
 *   opts.capabilitiesOverride {object?}  inject capabilities directly (for tests)
 * @returns {{ strategy: 'elicitation'|'local_presenter'|'text_fallback', reason: string }}
 */
function selectStrategy(rootDir, opts) {
  opts = opts || {};

  // Allow test override
  if (opts.capabilitiesOverride) {
    const caps = opts.capabilitiesOverride;
    if (caps.supports_elicitation) {
      return { strategy: "elicitation", reason: "capability override: elicitation enabled" };
    }
    if (caps.supports_local_presenter) {
      return { strategy: "local_presenter", reason: "capability override: local presenter available" };
    }
    return { strategy: "text_fallback", reason: "capability override: text fallback" };
  }

  // Signal 1: env var
  if (process.env.SEEYUE_MCP_ELICITATION === "1") {
    return { strategy: "elicitation", reason: "env SEEYUE_MCP_ELICITATION=1" };
  }

  // Signal 2: capabilities.yaml
  const capPath = path.join(rootDir, ".ai", "workflow", "capabilities.yaml");
  if (fs.existsSync(capPath)) {
    const content = fs.readFileSync(capPath, "utf8");
    if (content.split("\n").some(l => l.trim() === "elicitation: true")) {
      return { strategy: "elicitation", reason: "capabilities.yaml: elicitation: true" };
    }
  }

  // Signal 3: sy-interact binary present
  // Check both sy-interact/target/ (dedicated crate) and seeyue-mcp/target/ (workspace build)
  const REPO_ROOT = path.resolve(__dirname, "..", "..");
  const candidates = [
    path.join(REPO_ROOT, "sy-interact",  "target", "debug",   "sy-interact.exe"),
    path.join(REPO_ROOT, "sy-interact",  "target", "debug",   "sy-interact"),
    path.join(REPO_ROOT, "sy-interact",  "target", "release", "sy-interact.exe"),
    path.join(REPO_ROOT, "sy-interact",  "target", "release", "sy-interact"),
    path.join(REPO_ROOT, "seeyue-mcp",   "target", "debug",   "sy-interact.exe"),
    path.join(REPO_ROOT, "seeyue-mcp",   "target", "debug",   "sy-interact"),
    path.join(REPO_ROOT, "seeyue-mcp",   "target", "release", "sy-interact.exe"),
    path.join(REPO_ROOT, "seeyue-mcp",   "target", "release", "sy-interact"),
    path.join(REPO_ROOT, "target",       "debug",             "sy-interact.exe"),
    path.join(REPO_ROOT, "target",       "debug",             "sy-interact"),
    path.join(REPO_ROOT, "target",       "release",           "sy-interact.exe"),
    path.join(REPO_ROOT, "target",       "release",           "sy-interact"),
  ];
  if (candidates.some(c => fs.existsSync(c))) {
    return { strategy: "local_presenter", reason: "sy-interact binary found" };
  }

  return { strategy: "text_fallback", reason: "no elicitation or presenter available" };
}

// ─── Strategy handlers ────────────────────────────────────────────────────────

/**
 * Handle elicitation path.
 * In elicitation mode the MCP client resolves the interaction natively.
 * We do NOT write a response file — the client owns response lifecycle.
 *
 * @returns {DispatchResult}
 */
function handleElicitation(rootDir, interactionId) {
  return {
    strategy:   "elicitation",
    exitCode:   0,
    response:   {
      interaction_id: interactionId,
      strategy:       "elicitation",
      status:         "elicitation_pending",
      presenter:      "mcp_elicitation",
      dispatched_at:  new Date().toISOString(),
    },
    error:      null,
  };
}

/**
 * Handle text_fallback path.
 * Returns a structured result with instructions for plain-text prompt delivery.
 * Does NOT write to response store — pre-resolution states are not schema-valid responses.
 *
 * @returns {DispatchResult}
 */
function handleTextFallback(rootDir, interactionId) {
  const request = readRequest(rootDir, interactionId);
  return {
    strategy:   "text_fallback",
    exitCode:   0,
    response:   {
      interaction_id: interactionId,
      strategy:       "text_fallback",
      status:         "text_fallback_pending",
      presenter:      "text",
      title:          request ? request.title : interactionId,
      message:        request ? request.message : "",
      dispatched_at:  new Date().toISOString(),
    },
    error:      null,
  };
}

// ─── Main orchestrator ────────────────────────────────────────────────────────

/**
 * Orchestrate an interaction using the best available delivery strategy.
 *
 * @param {string} rootDir        — workspace root
 * @param {string} interactionId  — e.g. "ix-20260318-001"
 * @param {object} opts
 *   opts.capabilitiesOverride {object?}   inject strategy override (for tests)
 *   opts.binaryPath           {string?}   override sy-interact binary path
 *   opts.timeout              {number?}   timeout seconds for local_presenter
 * @returns {DispatchResult}
 *
 * @typedef {{ strategy: string, exitCode: number, response: object|null, error: string|null }} DispatchResult
 */
function orchestrateInteraction(rootDir, interactionId, opts) {
  opts = opts || {};

  const { strategy, reason } = selectStrategy(rootDir, opts);

  switch (strategy) {
    case "elicitation":
      return handleElicitation(rootDir, interactionId);

    case "local_presenter": {
      const result = dispatchInteraction(rootDir, interactionId, {
        binaryPath: opts.binaryPath,
        timeout:    opts.timeout,
      });
      return {
        strategy:  "local_presenter",
        exitCode:  result.exitCode,
        response:  result.response,
        error:     result.error,
      };
    }

    case "text_fallback":
    default:
      return handleTextFallback(rootDir, interactionId);
  }
}

module.exports = { orchestrateInteraction, selectStrategy };
