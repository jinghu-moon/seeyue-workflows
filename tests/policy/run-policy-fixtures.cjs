#!/usr/bin/env node
"use strict";

const path = require("node:path");
const { buildFixtureState, assertSubset, loadFixtureMap } = require("../runtime/runtime-fixture-lib.cjs");
const { loadWorkflowSpecs } = require("../../scripts/runtime/workflow-specs.cjs");
const { evaluatePolicy } = require("../../scripts/runtime/policy.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");
const fixtureDir = path.join(__dirname, "fixtures");
const cases = loadFixtureMap(fixtureDir);
const specs = loadWorkflowSpecs(projectRoot);

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
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }
  const selected = parsed.caseName ? [[parsed.caseName, cases.get(parsed.caseName)]] : [...cases.entries()];
  if (selected.some(([, value]) => !value)) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }
  let failed = false;
  for (const [caseName, fixture] of selected) {
    try {
      const input = buildFixtureState(fixture);
      const result = evaluatePolicy({
        session: input.session,
        taskGraph: input.taskGraph,
        actionContext: input.actionContext,
        specs,
      });
      assertSubset(result, input.expected, `case:${caseName}`);
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
  console.log("POLICY_FIXTURES_PASS");
}

main();
