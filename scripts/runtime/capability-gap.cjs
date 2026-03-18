"use strict";

// capability-gap.cjs — P2-N6: Adapter/Compiler Capability-Gap Mapping
//
// Maps each engine's native interaction capabilities against the unified
// interaction schema. Returns a gap descriptor telling callers whether a
// given interaction kind can be served natively or must fall back to
// sy-interact (presenter) or plain-text.
//
// Invariants:
//   - Node.js runtime is the sole decision authority.
//   - sy-interact is presenter-only; it never drives policy.
//   - Native capability is always preferred over sy-interact fallback.
//   - "gap" means: interaction_kind is NOT natively supported by the engine.

// ─── Engine capability registry ──────────────────────────────────────────────
//
// Each entry defines which interaction kinds the engine handles natively.
// Values:
//   "native"    — engine has first-class support; no presenter needed
//   "partial"   — engine can handle it but with limitations
//   "gap"       — no native support; presenter or text fallback required

const ENGINE_CAPABILITIES = {
  claude_code: {
    approval_request:     "native",   // PreToolUse permission + hook output
    restore_request:      "partial",  // Stop/PreCompact derived; richer UI via sy-interact
    question_request:     "partial",  // prompt/skill + hook assist; sy-interact for richer menu
    input_request:        "partial",  // prompt/skill; complex input needs sy-interact
    conflict_resolution:  "gap",      // hook returns block+reason; local menu via sy-interact
  },
  codex: {
    approval_request:     "partial",  // approval_policy + host; sy-interact primary local UI
    restore_request:      "gap",      // runtime + host; sy-interact primary
    question_request:     "gap",      // prompt/MCP/host; sy-interact for local menu
    input_request:        "gap",      // MCP/host; sy-interact for path/scope/params
    conflict_resolution:  "gap",      // no native lifecycle; sy-interact primary
  },
  gemini_cli: {
    approval_request:     "native",   // native approval UI
    restore_request:      "native",   // native checkpoint/restore semantics
    question_request:     "partial",  // native but sy-interact supplements conflict/input
    input_request:        "partial",  // native; sy-interact for supplemental input
    conflict_resolution:  "partial",  // native; sy-interact for supplemental conflict menu
  },
};

// ─── Fallback policy ─────────────────────────────────────────────────────────
//
// When a capability is "gap" or "partial", determine the preferred fallback.
// Priority: local_presenter (sy-interact) > text_fallback

const FALLBACK_POLICY = {
  native:  "none",
  partial: "local_presenter",
  gap:     "local_presenter",
};

// ─── Known interaction kinds ──────────────────────────────────────────────────

const KNOWN_INTERACTION_KINDS = [
  "approval_request",
  "restore_request",
  "question_request",
  "input_request",
  "conflict_resolution",
];

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Returns full capability gap map for the given engine.
 *
 * @param {string} engine — "claude_code" | "codex" | "gemini_cli"
 * @returns {object} { engine, capabilities: { [kind]: { native_level, fallback, has_gap } } }
 */
function getEngineCapabilityMap(engine) {
  const caps = ENGINE_CAPABILITIES[engine];
  if (!caps) {
    throw new Error(
      `capability-gap: unknown engine '${engine}'. Valid: ${Object.keys(ENGINE_CAPABILITIES).join(", ")}`
    );
  }

  const capabilities = {};
  for (const kind of KNOWN_INTERACTION_KINDS) {
    const native_level = caps[kind] || "gap";
    capabilities[kind] = {
      native_level,
      fallback: FALLBACK_POLICY[native_level] || "text_fallback",
      has_gap: native_level !== "native",
    };
  }

  return { engine, capabilities };
}

/**
 * Returns the fallback mode for a specific interaction kind on an engine.
 *
 * @param {string} engine          — engine identifier
 * @param {string} interaction_kind — interaction kind
 * @param {boolean} presenterAvailable — whether sy-interact binary is available
 * @returns {string} "none" | "local_presenter" | "text_fallback"
 */
function resolveFallback(engine, interaction_kind, presenterAvailable) {
  const caps = ENGINE_CAPABILITIES[engine];
  if (!caps) {
    return presenterAvailable ? "local_presenter" : "text_fallback";
  }

  const native_level = caps[interaction_kind] || "gap";
  if (native_level === "native") {
    return "none";
  }

  if (presenterAvailable) {
    return "local_presenter";
  }

  return "text_fallback";
}

/**
 * Returns all gaps (non-native kinds) for the given engine.
 *
 * @param {string} engine — engine identifier
 * @returns {string[]} list of interaction kinds that are not natively supported
 */
function getGapKinds(engine) {
  const map = getEngineCapabilityMap(engine);
  return Object.entries(map.capabilities)
    .filter(([, v]) => v.has_gap)
    .map(([kind]) => kind);
}

/**
 * Returns a summary gap report for all known engines.
 *
 * @returns {object} { engines: { [engine]: { gap_count, gap_kinds } } }
 */
function getAllEngineGapReport() {
  const engines = {};
  for (const engine of Object.keys(ENGINE_CAPABILITIES)) {
    const gaps = getGapKinds(engine);
    engines[engine] = {
      gap_count: gaps.length,
      gap_kinds: gaps,
    };
  }
  return { engines };
}

module.exports = {
  getAllEngineGapReport,
  getEngineCapabilityMap,
  getGapKinds,
  resolveFallback,
  KNOWN_INTERACTION_KINDS,
  ENGINE_CAPABILITIES,
};
