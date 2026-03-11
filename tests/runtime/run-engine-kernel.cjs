#!/usr/bin/env node
"use strict";

const path = require("node:path");
const { appendJournalEvents, writeSession, writeSprintStatus, writeTaskGraph, readSprintStatus } = require("../../scripts/runtime/store.cjs");
const { runEngineKernel } = require("../../scripts/runtime/engine-kernel.cjs");
const { buildFixtureState, assertSubset, loadFixtureMap, makeTempRoot } = require("./runtime-fixture-lib.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");
const fixtureDir = path.join(__dirname, "fixtures");
const cases = loadFixtureMap(fixtureDir);

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
      const rootDir = makeTempRoot("sy-engine-kernel-");
      writeSession(rootDir, input.session);
      writeTaskGraph(rootDir, input.taskGraph);
      writeSprintStatus(rootDir, input.sprintStatus);
      if (Array.isArray(input.journal) && input.journal.length > 0) {
        appendJournalEvents(rootDir, input.journal);
      }
      const result = runEngineKernel(rootDir, {
        actionContext: input.actionContext,
        now: input.now || undefined,
        syncSprintStatus: true,
        specRootDir: projectRoot,
      });
      assertSubset(result, input.expected.result || input.expected, `case:${caseName}.result`);
      if (input.expected.sprint_status) {
        assertSubset(readSprintStatus(rootDir), input.expected.sprint_status, `case:${caseName}.sprint_status`);
      }
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
  console.log("ENGINE_KERNEL_PASS");
}

main();
