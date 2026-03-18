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

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
}

function writeVerifyStaging(rootDir, phases) {
  writeJson(path.join(rootDir, ".ai", "analysis", "verify-staging.json"), {
    updated_at: "2026-03-09T08:00:00.000Z",
    session_run_id: "wf-20260308-101",
    phases,
  });
}

function writeCoverageStaging(rootDir, overrides = {}) {
  writeJson(path.join(rootDir, ".ai", "analysis", "coverage-staging.json"), {
    schema_version: 1,
    adapter_kind: "coverage_adapter",
    source_format: "normalized_json",
    source_ref: "coverage/coverage-summary.json",
    session_run_id: "wf-20260308-101",
    node_id: "P2-N2",
    task_mode: "feature",
    coverage_mode: "full",
    coverage_profile: "standard",
    required: 80,
    actual: 86,
    actual_basis: "lines_pct",
    metrics: {
      lines_pct: 86,
      statements_pct: 85,
      functions_pct: 84,
      branches_pct: 83,
    },
    globalRegressed: false,
    touchedRegionRegressed: false,
    characterizationAdded: false,
    pass: true,
    status: "pass",
    reasons: [],
    recorded_at: "2026-03-09T08:05:00.000Z",
    ...overrides,
  });
}

function runNodeScript(rootDir, relativeScriptPath, args = []) {
  const scriptPath = path.join(rootDir, relativeScriptPath);
  return spawnSync(process.execPath, [scriptPath, ...args], {
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

function loadReport(rootDir) {
  return JSON.parse(fs.readFileSync(path.join(rootDir, ".ai", "analysis", "ai.report.json"), "utf8"));
}

const cases = {
  "build-report-from-staging-ready": () => {
    const rootDir = makeTempRoot("report-builder-ready-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N1": { status: "completed", tdd_state: "verified" },
        "P2-N2": {
          status: "review",
          tdd_required: false,
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: [".ai/analysis/verify-staging.json"],
        },
        "P2-N3": { status: "failed", tdd_state: "green_verified" },
      },
    });
    writeVerifyStaging(rootDir, {
      build: {
        command: "npm run build",
        exit_code: 0,
        status: "pass",
        key_signal: "build ok",
        ts: "2026-03-09T08:00:00.000Z",
      },
      typecheck: {
        command: "npm run typecheck",
        exit_code: 0,
        status: "pass",
        key_signal: "typecheck ok",
        ts: "2026-03-09T08:01:00.000Z",
      },
      test: {
        command: "npm run test:runtime:p2",
        exit_code: 0,
        status: "pass",
        key_signal: "ENGINE_KERNEL_PASS",
        ts: "2026-03-09T08:02:00.000Z",
      },
    });
    writeCoverageStaging(rootDir);

    const result = parseJsonOutput(
      runNodeScript(rootDir, path.join("scripts", "runtime", "report-builder.cjs"), ["--root", rootDir, "--write", "--json"]),
      "report-builder",
    );
    assertSubset(result, {
      overall: "READY",
      report_ref: ".ai/analysis/ai.report.json",
      summary: {
        total_nodes: 4,
        completed_nodes: 2,
        failed_nodes: 1,
      },
      verification: {
        build: { status: "pass" },
        typecheck: { status: "pass" },
        coverage: { status: "pass", actual: 86, required: 80 },
      },
    });

    const report = loadReport(rootDir);
    assertSubset(report, {
      overall: "READY",
      summary: {
        total_nodes: 4,
        completed_nodes: 2,
        failed_nodes: 1,
      },
      verification: {
        build: { status: "pass" },
        typecheck: { status: "pass" },
        coverage: { status: "pass", actual: 86, required: 80 },
      },
    });
    if (!Array.isArray(report.verification.tests) || report.verification.tests[0]?.status !== "pass") {
      throw new Error("expected verification.tests[0].status=pass");
    }
  },
  "build-report-from-staging-not-ready": () => {
    const rootDir = makeTempRoot("report-builder-fail-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N2": {
          status: "review",
          tdd_required: false,
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });
    writeVerifyStaging(rootDir, {
      test: {
        command: "npm run test:runtime:p2",
        exit_code: 1,
        status: "fail",
        key_signal: "ENGINE_KERNEL_FAIL",
        ts: "2026-03-09T08:03:00.000Z",
      },
    });
    writeCoverageStaging(rootDir, {
      pass: false,
      status: "fail",
      reasons: ["coverage_below_threshold"],
      actual: 71,
      required: 80,
    });

    const result = parseJsonOutput(
      runNodeScript(rootDir, path.join("scripts", "runtime", "report-builder.cjs"), ["--root", rootDir, "--write", "--json"]),
      "report-builder",
    );
    assertSubset(result, {
      overall: "NOT_READY",
      verification: {
        tests: [{ status: "fail" }],
      },
    });
  },
  "controller-verify-write-report": () => {
    const rootDir = makeTempRoot("controller-write-report-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N2": {
          status: "review",
          tdd_required: false,
          tdd_state: "verified",
          test_contract: null,
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });
    writeVerifyStaging(rootDir, {
      lint: {
        command: "npm run lint",
        exit_code: 0,
        status: "pass",
        key_signal: "lint ok",
        ts: "2026-03-09T08:04:00.000Z",
      },
    });
    writeCoverageStaging(rootDir);

    const result = parseJsonOutput(
      runNodeScript(rootDir, path.join("scripts", "runtime", "controller.cjs"), ["--root", rootDir, "--mode", "verify", "--write-report", "--json"]),
      "controller",
    );
    assertSubset(result, {
      mode: "verify",
      verification: {
        report_exists: true,
        report_ref: ".ai/analysis/ai.report.json",
        report_overall: "READY",
        review_ready: true,
      },
    });
  },
  "build-report-from-existing-report-fallback": () => {
    const rootDir = makeTempRoot("report-builder-fallback-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N2": {
          status: "review",
          tdd_required: false,
          tdd_state: "verified",
          test_contract: null,
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });

    writeJson(path.join(rootDir, ".ai", "analysis", "ai.report.json"), {
      report_name: "ai.report",
      report_version: "v4",
      generated_at: "2026-03-09T08:00:00.000Z",
      updated_at: "2026-03-09T08:00:00.000Z",
      summary: { task: "fallback" },
      verification: {
        tests: [
          {
            command: "npm run test:runtime:p2",
            exit_code: 0,
            key_signal: "ENGINE_KERNEL_PASS",
          },
        ],
      },
      overall: "NOT_READY",
    });

    const result = parseJsonOutput(
      runNodeScript(rootDir, path.join("scripts", "runtime", "report-builder.cjs"), ["--root", rootDir, "--write", "--json"]),
      "report-builder",
    );
    assertSubset(result, {
      overall: "READY",
      verification: {
        tests: [{ status: "pass" }],
      },
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
    console.log("REPORT_BUILDER_FIXTURES_PASS");
  }
}

main();
