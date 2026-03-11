#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const {
  appendOutputLog,
  readOutputLog,
} = require("../../scripts/runtime/output-log.cjs");
const {
  validateOutputLogFile,
} = require("../../scripts/runtime/validate-output.cjs");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-output-log-"));
}

const cases = {
  "append-and-read": () => {
    const rootDir = makeTempRoot();
    const repoRoot = path.resolve(__dirname, "..", "..");
    appendOutputLog(rootDir, {
      template_id: "status-indicator",
      output_level: "summary",
      variables: { status: "ok", message: "ready" },
    });
    const entries = readOutputLog(rootDir);
    assert(entries.length === 1, "expected one output log entry");
    assert(entries[0].template_id === "status-indicator", "template id mismatch");
  },
  "validate-log-file": () => {
    const rootDir = makeTempRoot();
    const repoRoot = path.resolve(__dirname, "..", "..");
    appendOutputLog(rootDir, {
      template_id: "status-indicator",
      output_level: "summary",
      variables: { status: "ok", message: "ready" },
    });
    const logPath = path.join(rootDir, ".ai", "workflow", "output.log");
    const result = validateOutputLogFile(repoRoot, logPath);
    assert(result.ok, "expected output log validation to pass");
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

console.log("OUTPUT_LOG_PASS");
