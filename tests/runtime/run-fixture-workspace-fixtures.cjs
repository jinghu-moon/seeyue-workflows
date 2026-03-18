#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const {
  copyRuntimeFixtureFiles,
  cleanupTrackedTempRoots,
  makeTempRoot,
} = require("./runtime-fixture-lib.cjs");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const cases = {
  "runtime-fixture-copy-excludes-heavy-directories": () => {
    const rootDir = makeTempRoot("runtime-fixture-copy-");
    copyRuntimeFixtureFiles(rootDir);

    assert(fs.existsSync(path.join(rootDir, "scripts")), "expected scripts/ to be copied");
    assert(fs.existsSync(path.join(rootDir, "workflow")), "expected workflow/ to be copied");

    for (const forbidden of ["seeyue-mcp", "refer", ".git", "target"]) {
      assert(!fs.existsSync(path.join(rootDir, forbidden)), `expected ${forbidden} to stay out of temp root`);
    }
  },
  "tracked-temp-roots-are-cleaned": () => {
    const rootDir = makeTempRoot("runtime-fixture-cleanup-");
    assert(fs.existsSync(rootDir), "expected temp root to exist before cleanup");
    cleanupTrackedTempRoots();
    assert(!fs.existsSync(rootDir), "expected tracked temp root to be deleted by cleanup");
  },
};

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

  if (selected.some(([, value]) => typeof value !== "function")) {
    throw new Error(`Unknown case: ${parsed.caseName}`);
  }

  for (const [caseName, executeCase] of selected) {
    executeCase();
    console.log(`CASE_PASS ${caseName}`);
  }

  console.log("FIXTURE_WORKSPACE_FIXTURES_PASS");
}

main();
