#!/usr/bin/env node
"use strict";

const path = require("node:path");
const { validateWorkflowSpecs } = require("./spec-validator.cjs");

function parseArgs(argv) {
  const result = {
    rootDir: path.resolve(__dirname, "..", ".."),
    specPaths: [],
    validateAll: false,
    validateManifestOnly: false,
    freezeGate: null,
    validateScope: "full",
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--root") {
      index += 1;
      result.rootDir = path.resolve(argv[index]);
      continue;
    }
    if (token === "--scope") {
      index += 1;
      result.validateScope = argv[index];
      continue;
    }
    if (token === "--gate") {
      index += 1;
      result.freezeGate = argv[index];
      continue;
    }
    if (token === "--spec") {
      index += 1;
      result.specPaths.push(argv[index].replace(/\\/g, "/"));
      continue;
    }
    if (token === "--all") {
      result.validateAll = true;
      continue;
    }
    if (token === "--manifest") {
      result.validateManifestOnly = true;
      continue;
    }
    throw new Error(`Unknown argument: ${token}`);
  }
  return result;
}

function main() {
  let args;
  try {
    args = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  try {
    const result = validateWorkflowSpecs({ ...args, validateScope: args.validateScope });
    if (!result.ok) {
      for (const issue of result.issues) {
        const prefix = issue.severity === "warning" ? "WARN" : "ERROR";
        console.error(`${prefix} ${issue.code} ${issue.specPath} ${issue.message}`);
      }
      console.error("SPEC_VALIDATION_FAIL");
      process.exit(1);
    }
    for (const issue of result.issues) {
      if (issue.severity === "warning") {
        console.error(`WARN ${issue.code} ${issue.specPath} ${issue.message}`);
      }
    }
    console.log(`Validated ${result.specsValidated.length} spec files under ${result.rootDir}`);
    console.log("SPEC_VALIDATION_PASS");
  } catch (error) {
    console.error(`SPEC_VALIDATION_CRASH ${error.message}`);
    process.exit(1);
  }
}

main();
