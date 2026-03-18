#!/usr/bin/env node
"use strict";

// tests/hooks/run-capability-gap-fixtures.cjs
// P2-N6: Capability-gap mapping fixtures

const fs = require("node:fs");
const path = require("node:path");

const projectRoot = path.resolve(__dirname, "..", "..");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── Case: module-loadable ───────────────────────────────────────────────────
// GREEN: capability-gap.cjs loads and exports required functions

function runModuleLoadable() {
  const modulePath = path.join(projectRoot, "scripts", "runtime", "capability-gap.cjs");
  assert(fs.existsSync(modulePath), `capability-gap.cjs not found at ${modulePath}`);

  const mod = require(modulePath);
  assert(typeof mod.getEngineCapabilityMap === "function", "must export getEngineCapabilityMap");
  assert(typeof mod.resolveFallback === "function", "must export resolveFallback");
  assert(typeof mod.getGapKinds === "function", "must export getGapKinds");
  assert(typeof mod.getAllEngineGapReport === "function", "must export getAllEngineGapReport");
  assert(Array.isArray(mod.KNOWN_INTERACTION_KINDS), "must export KNOWN_INTERACTION_KINDS array");
  assert(typeof mod.ENGINE_CAPABILITIES === "object", "must export ENGINE_CAPABILITIES object");
}

// ─── Case: all-engines-covered ──────────────────────────────────────────────
// GREEN: all three engines are present and define capabilities for all known kinds

function runAllEnginesCovered() {
  const { ENGINE_CAPABILITIES, KNOWN_INTERACTION_KINDS } = require(
    path.join(projectRoot, "scripts", "runtime", "capability-gap.cjs")
  );

  const requiredEngines = ["claude_code", "codex", "gemini_cli"];
  for (const engine of requiredEngines) {
    assert(
      engine in ENGINE_CAPABILITIES,
      `ENGINE_CAPABILITIES must include '${engine}'`
    );
    const caps = ENGINE_CAPABILITIES[engine];
    for (const kind of KNOWN_INTERACTION_KINDS) {
      assert(
        kind in caps,
        `Engine '${engine}' must define capability for '${kind}'`
      );
      const validLevels = ["native", "partial", "gap"];
      assert(
        validLevels.includes(caps[kind]),
        `Engine '${engine}' kind '${kind}' must be native|partial|gap, got '${caps[kind]}'`
      );
    }
  }
}

// ─── Case: resolve-fallback-logic ────────────────────────────────────────────
// GREEN: resolveFallback returns correct values for each scenario

function runResolveFallbackLogic() {
  const { resolveFallback } = require(
    path.join(projectRoot, "scripts", "runtime", "capability-gap.cjs")
  );

  // Native capability -> no fallback needed
  const nativeResult = resolveFallback("claude_code", "approval_request", false);
  assert(nativeResult === "none", `native should return 'none', got '${nativeResult}'`);

  // Gap + presenter available -> local_presenter
  const gapWithPresenter = resolveFallback("codex", "restore_request", true);
  assert(
    gapWithPresenter === "local_presenter",
    `gap+presenter should return 'local_presenter', got '${gapWithPresenter}'`
  );

  // Gap + no presenter -> text_fallback
  const gapNoPresenter = resolveFallback("codex", "restore_request", false);
  assert(
    gapNoPresenter === "text_fallback",
    `gap+no presenter should return 'text_fallback', got '${gapNoPresenter}'`
  );

  // Unknown engine -> graceful fallback
  const unknownEngine = resolveFallback("unknown_engine", "approval_request", true);
  assert(
    unknownEngine === "local_presenter",
    `unknown engine + presenter should return 'local_presenter', got '${unknownEngine}'`
  );
}

// ─── Case: get-gap-kinds ─────────────────────────────────────────────────────
// GREEN: getGapKinds returns expected gaps per engine

function runGetGapKinds() {
  const { getGapKinds } = require(
    path.join(projectRoot, "scripts", "runtime", "capability-gap.cjs")
  );

  // claude_code: only conflict_resolution is a full gap
  const claudeGaps = getGapKinds("claude_code");
  assert(Array.isArray(claudeGaps), "getGapKinds must return an array");
  assert(
    claudeGaps.includes("conflict_resolution"),
    "claude_code must have conflict_resolution as a gap"
  );
  // approval_request is native for claude_code, must NOT be in gaps
  assert(
    !claudeGaps.includes("approval_request"),
    "claude_code approval_request is native, must not appear in gaps"
  );

  // codex: everything is gap or partial — at least 3 gaps
  const codexGaps = getGapKinds("codex");
  assert(codexGaps.length >= 3, `codex should have >= 3 gaps, got ${codexGaps.length}`);

  // gemini_cli: approval and restore are native — fewer gaps than codex
  const geminiGaps = getGapKinds("gemini_cli");
  assert(
    geminiGaps.length < codexGaps.length,
    `gemini_cli should have fewer gaps than codex (${geminiGaps.length} vs ${codexGaps.length})`
  );
}

// ─── Case: gap-report-shape ──────────────────────────────────────────────────
// GREEN: getAllEngineGapReport returns valid shape

function runGapReportShape() {
  const { getAllEngineGapReport } = require(
    path.join(projectRoot, "scripts", "runtime", "capability-gap.cjs")
  );

  const report = getAllEngineGapReport();
  assert(typeof report === "object" && report !== null, "report must be an object");
  assert(typeof report.engines === "object", "report must have engines key");

  for (const [engine, info] of Object.entries(report.engines)) {
    assert(typeof info.gap_count === "number", `${engine}.gap_count must be a number`);
    assert(Array.isArray(info.gap_kinds), `${engine}.gap_kinds must be an array`);
    assert(
      info.gap_kinds.length === info.gap_count,
      `${engine}.gap_count must match gap_kinds.length`
    );
  }
}

// ─── Runner ───────────────────────────────────────────────────────────────────

const CASES = {
  "module-loadable":      runModuleLoadable,
  "all-engines-covered":  runAllEnginesCovered,
  "resolve-fallback-logic": runResolveFallbackLogic,
  "get-gap-kinds":        runGetGapKinds,
  "gap-report-shape":     runGapReportShape,
};

function main() {
  const caseArg = process.argv.indexOf("--case");
  const caseName = caseArg !== -1 ? process.argv[caseArg + 1] : null;

  let failed = false;

  if (caseName) {
    const runner = CASES[caseName];
    if (!runner) {
      console.error(`Unknown case: ${caseName}`);
      console.error(`Available: ${Object.keys(CASES).join(", ")}`);
      process.exit(1);
    }
    try {
      runner();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
    }
  } else {
    for (const [name, runner] of Object.entries(CASES)) {
      try {
        runner();
        console.log(`CASE_PASS ${name}`);
      } catch (error) {
        failed = true;
        console.error(`CASE_FAIL ${name}`);
        console.error(error.stack || error.message);
      }
    }
  }

  if (failed) {
    process.exit(1);
  }
}

main();
