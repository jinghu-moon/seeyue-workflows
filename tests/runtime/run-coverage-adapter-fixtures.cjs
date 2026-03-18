"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const {
  buildFixtureState,
  assertSubset,
  copyRuntimeFixtureFiles,
  makeTempRoot,
} = require("./runtime-fixture-lib.cjs");
const {
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("../../scripts/runtime/store.cjs");

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  return state;
}

function writeText(filePath, content) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, content, "utf8");
}

function writeJson(filePath, value) {
  writeText(filePath, JSON.stringify(value, null, 2));
}

function runAdapter(rootDir, args) {
  const adapterPath = path.join(rootDir, "scripts", "runtime", "coverage-adapter.cjs");
  return spawnSync(process.execPath, [adapterPath, ...args], {
    cwd: rootDir,
    encoding: "utf8",
  });
}

function parseJsonOutput(result, label) {
  if (result.status !== 0) {
    throw new Error(`${label} exited with ${result.status}: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout || "{}");
}

const cases = {
  "missing-coverage-input-fails": () => {
    const rootDir = makeTempRoot("coverage-adapter-missing-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {});
    const result = runAdapter(rootDir, ["--root", rootDir, "--write", "--json"]);
    if (result.status === 0) {
      throw new Error("expected missing coverage input to fail");
    }
    if (!(result.stderr || result.stdout).includes("COVERAGE_INPUT_NOT_FOUND")) {
      throw new Error("expected COVERAGE_INPUT_NOT_FOUND signal");
    }
  },
  "istanbul-summary-write-staging": () => {
    const rootDir = makeTempRoot("coverage-adapter-istanbul-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {});
    writeJson(path.join(rootDir, "coverage", "coverage-summary.json"), {
      total: {
        lines: { pct: 91 },
        statements: { pct: 90 },
        functions: { pct: 89 },
        branches: { pct: 88 },
      },
    });

    const result = parseJsonOutput(
      runAdapter(rootDir, ["--root", rootDir, "--write", "--json"]),
      "coverage-adapter",
    );

    assertSubset(result, {
      source_format: "istanbul_summary",
      coverage_mode: "full",
      coverage_profile: "standard",
      actual: 91,
      required: 80,
      pass: true,
      status: "pass",
      staging_ref: ".ai/analysis/coverage-staging.json",
    });
  },
  "cobertura-patch-regression-fails": () => {
    const rootDir = makeTempRoot("coverage-adapter-cobertura-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: { task: { mode: "bugfix" } },
      nodes: {
        "P2-N1": {
          test_contract: {
            layer: "integration",
            coverage_mode: "patch",
            coverage_profile: "standard",
            mock_policy: "boundary_only",
            acceptance_criteria_refs: ["AC-1"],
            red_cmd: "node red P2-N1",
            green_cmd: "node green P2-N1",
            red_expectation: {
              allowed_failure_kinds: ["assertion_failure"],
              rejected_failure_kinds: ["syntax_error"],
              allowed_exit_codes: [1],
              stderr_pattern: null,
              error_type: null,
            },
            behavior_gate: {
              ac_traceability_required: true,
              boundary_conditions_required: true,
            },
          },
        },
      },
    });
    writeText(
      path.join(rootDir, "coverage.xml"),
      "<?xml version=\"1.0\" ?><coverage line-rate=\"0.87\" branch-rate=\"0.66\"></coverage>",
    );

    const result = parseJsonOutput(
      runAdapter(rootDir, [
        "--root",
        rootDir,
        "--write",
        "--json",
        "--coverage-mode",
        "patch",
        "--coverage-profile",
        "standard",
        "--global-regressed",
        "true",
        "--characterization-added",
        "true",
      ]),
      "coverage-adapter",
    );

    assertSubset(result, {
      source_format: "cobertura_xml",
      coverage_mode: "patch",
      coverage_profile: "standard",
      actual: 87,
      required: 80,
      pass: false,
      status: "fail",
    });

    if (!Array.isArray(result.reasons) || !result.reasons.includes("global_coverage_regressed")) {
      throw new Error("expected global_coverage_regressed reason");
    }
  },
  "auto-discovers-active-node-contract": () => {
    const rootDir = makeTempRoot("coverage-adapter-contract-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        task: { mode: "bugfix" },
        node: { active_id: "P2-N1", state: "green_verified", owner_persona: "author" },
      },
      nodes: {
        "P2-N1": {
          test_contract: {
            layer: "integration",
            coverage_mode: "patch",
            coverage_profile: "core",
            mock_policy: "boundary_only",
            acceptance_criteria_refs: ["AC-1"],
            red_cmd: "node red P2-N1",
            green_cmd: "node green P2-N1",
            red_expectation: {
              allowed_failure_kinds: ["assertion_failure"],
              rejected_failure_kinds: ["syntax_error"],
              allowed_exit_codes: [1],
              stderr_pattern: null,
              error_type: null,
            },
            behavior_gate: {
              ac_traceability_required: true,
              boundary_conditions_required: true,
            },
          },
        },
      },
    });
    writeJson(path.join(rootDir, "coverage", "coverage-summary.json"), {
      total: {
        lines: { pct: 93 },
        statements: { pct: 92 },
        functions: { pct: 91 },
        branches: { pct: 90 },
      },
    });

    const result = parseJsonOutput(
      runAdapter(rootDir, [
        "--root",
        rootDir,
        "--write",
        "--json",
        "--touched-region-regressed",
        "false",
        "--characterization-added",
        "true",
      ]),
      "coverage-adapter",
    );

    assertSubset(result, {
      coverage_mode: "patch",
      coverage_profile: "core",
      required: 90,
      actual: 93,
      pass: true,
      status: "pass",
    });
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
  if (selected.some(([, run]) => typeof run !== "function")) {
    throw new Error(`Unknown case: ${parsed.caseName}`);
  }
  for (const [caseName, run] of selected) {
    try {
      run();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
      process.exit(1);
    }
  }
  if (!parsed.caseName) {
    console.log("COVERAGE_ADAPTER_FIXTURES_PASS");
  }
}

main();
