#!/usr/bin/env node
"use strict";

const path = require("node:path");
const {
  validateOutputEntries,
  loadOutputTemplates,
} = require("../../scripts/runtime/validate-output.cjs");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function makeEntry(templateId, variables, outputLevel = "detail") {
  return {
    template_id: templateId,
    output_level: outputLevel,
    variables,
  };
}

const rootDir = path.resolve(__dirname, "..", "..");
const templates = loadOutputTemplates(rootDir);

const cases = {
  "valid-checkpoint-entry": () => {
    const entry = makeEntry("checkpoint", {
      node_id: "P1-N4",
      file_path: "scripts/runtime/store.cjs",
      line_start: 1,
      line_end: 10,
      lines_added: 3,
      lines_removed: 1,
      verify_command: "node tests/runtime/run-runtime-store.cjs",
      tests_passed: true,
      tests_failed: false,
      duration: "3s",
      scope_status: "ok",
      scope_detail: "store.cjs only",
      constraints_status: "ok",
      constraints_detail: "n/a",
      tdd_status: "n/a",
      tdd_detail: "not required",
      current_node: 1,
      total_nodes: 3,
      progress_pct: 33,
      next_action: "continue",
    });
    const result = validateOutputEntries([entry], templates);
    assert(result.ok, "expected valid checkpoint entry");
  },
  "missing-required-variable": () => {
    const entry = makeEntry("checkpoint", {
      node_id: "P1-N4",
    });
    const result = validateOutputEntries([entry], templates);
    assert(!result.ok, "expected validation to fail for missing variables");
  },
  "unknown-template": () => {
    const entry = makeEntry("unknown_template", { status: "ok" });
    const result = validateOutputEntries([entry], templates);
    assert(!result.ok, "expected validation to fail for unknown template");
  },
  "output-level-mismatch": () => {
    const entry = makeEntry("status-indicator", { status: "ok", message: "ready" }, "detail");
    const result = validateOutputEntries([entry], templates);
    assert(!result.ok, "expected validation to fail for output level mismatch");
  },
};

let failed = false;
for (const [name, run] of Object.entries(cases)) {
  try {
    run();
    console.log(`CASE_PASS ${name}`);
  } catch (error) {
    failed = true;
    console.error(`CASE_FAIL ${name}`);
    console.error(error.stack || error.message);
  }
}

if (failed) {
  process.exit(1);
}

console.log("OUTPUT_TEMPLATES_PASS");
