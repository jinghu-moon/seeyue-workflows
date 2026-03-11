#!/usr/bin/env node
"use strict";

const path = require("node:path");

const {
  assertApprovalCopyAligned,
  assertCoreSurfaceAlignment,
  assertDangerousCommandGuard,
  assertResumeFrontierAlignment,
  assertTddWriteGuard,
  loadAllEngineArtifacts,
} = require("../../scripts/runtime/engine-conformance.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parseArgs(argv) {
  const parsed = {
    caseName: null,
    runAll: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--case":
        index += 1;
        parsed.caseName = argv[index];
        break;
      case "--all":
        parsed.runAll = true;
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }

  if (!parsed.caseName && !parsed.runAll) {
    parsed.runAll = true;
  }

  return parsed;
}

function buildDriftedArtifacts() {
  const artifacts = loadAllEngineArtifacts(projectRoot);
  const drifted = structuredClone(artifacts);
  drifted.codex.bundle.language_policy = {
    ...drifted.codex.bundle.language_policy,
    human_output_language: "en",
  };
  return drifted;
}

const cases = {
  "approval-copy-drift": () => {
    let failed = false;
    try {
      assertApprovalCopyAligned(buildDriftedArtifacts());
    } catch (error) {
      failed = true;
      assert(
        /ENGINE_APPROVAL_COPY_DRIFT/i.test(String(error.message || "")),
        `expected approval drift failure but got ${JSON.stringify(error.message)}`,
      );
    }
    assert(failed === true, "expected approval copy drift to be detected");
  },
  "approval-copy-aligned": () => {
    const artifacts = loadAllEngineArtifacts(projectRoot);
    assertApprovalCopyAligned(artifacts);
  },
  "guard-surfaces-aligned": () => {
    const artifacts = loadAllEngineArtifacts(projectRoot);
    assertCoreSurfaceAlignment(artifacts, {
      requiredChecks: ["language_policy", "dangerous_command_surface", "human_blocker_surface", "tdd_guard_surface"],
    });
    assertDangerousCommandGuard(projectRoot);
    assertTddWriteGuard(projectRoot);
  },
  "resume-frontier-aligned": () => {
    const artifacts = loadAllEngineArtifacts(projectRoot);
    assertCoreSurfaceAlignment(artifacts, {
      requiredChecks: ["resume_surface", "review_chain", "instruction_notice"],
    });
    assertResumeFrontierAlignment(projectRoot);
  },
  "adapter-output-consistency": () => {
    const artifacts = loadAllEngineArtifacts(projectRoot);
    assertCoreSurfaceAlignment(artifacts, {
      requiredChecks: ["language_policy", "instruction_notice", "render_targets", "review_chain", "human_blocker_surface"],
    });
  },
};

const defaultCases = [
  "approval-copy-aligned",
  "guard-surfaces-aligned",
  "resume-frontier-aligned",
  "adapter-output-consistency",
];

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  const selected = parsed.caseName ? [parsed.caseName] : defaultCases;
  for (const caseName of selected) {
    const runner = cases[caseName];
    if (typeof runner !== "function") {
      console.error(`UNKNOWN_CASE ${caseName}`);
      process.exit(1);
    }
    try {
      runner();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
      process.exit(1);
    }
  }

  if (!parsed.caseName) {
    console.log("ENGINE_CONFORMANCE_PASS");
  }
}

main();
