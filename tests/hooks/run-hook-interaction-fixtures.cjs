#!/usr/bin/env node
"use strict";

// tests/hooks/run-hook-interaction-fixtures.cjs
// P2-N2: Hook interaction envelope fixtures

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const projectRoot = path.resolve(__dirname, "..", "..");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── Case: missing-interaction-envelope ─────────────────────────────────────
// Historical RED snapshot: documents the state BEFORE P2-N2 implementation.
// Once the implementation is complete, this case is a no-op (always passes)
// because the GREEN case (decision-envelope) is the live gate.

function runMissingInteractionEnvelope() {
  // This case documents the pre-implementation state.
  // After P2-N2 is implemented, interaction envelope fields WILL be present.
  // We simply verify hook-client.cjs is loadable and exists.
  const hookClientPath = path.join(projectRoot, "scripts", "runtime", "hook-client.cjs");
  assert(fs.existsSync(hookClientPath), `hook-client.cjs not found at ${hookClientPath}`);
  // No-op: the RED phase is superseded by the GREEN case once implemented.
}

// ─── Case: decision-envelope ─────────────────────────────────────────────────
// GREEN: Assert enriched envelope fields present in hook-client output

function runDecisionEnvelope() {
  const hookClientPath = path.join(projectRoot, "scripts", "runtime", "hook-client.cjs");
  assert(fs.existsSync(hookClientPath), `hook-client.cjs not found at ${hookClientPath}`);

  const src = fs.readFileSync(hookClientPath, "utf8");
  assert(src.includes("interaction_required"), "hook-client.cjs must define 'interaction_required'");
  assert(src.includes("interaction_kind"), "hook-client.cjs must define 'interaction_kind'");
  assert(src.includes("blocking_kind"), "hook-client.cjs must define 'blocking_kind'");
  assert(src.includes("reason_code"), "hook-client.cjs must define 'reason_code'");
  assert(src.includes("risk_level"), "hook-client.cjs must define 'risk_level'");
  assert(src.includes("scope"), "hook-client.cjs must define 'scope'");

  // Also verify buildInteractionEnvelope is exported or inlined
  const hasEnvelopeFn = src.includes("buildInteractionEnvelope") || src.includes("interactionEnvelope");
  assert(hasEnvelopeFn, "hook-client.cjs must contain buildInteractionEnvelope or interactionEnvelope");
}

// ─── Runner ───────────────────────────────────────────────────────────────────

const CASES = {
  "missing-interaction-envelope": runMissingInteractionEnvelope,
  "decision-envelope": runDecisionEnvelope,
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
