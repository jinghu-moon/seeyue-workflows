#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const {
  buildFixtureState,
  assertSubset,
} = require("./runtime-fixture-lib.cjs");
const {
  readJournalEvents,
  readLedger,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeCheckpoint,
  writeLedger,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
  writeCapsule,
} = require("../../scripts/runtime/store.cjs");

function makeTempRoot(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  return state;
}

function parseJsonOutput(result, label) {
  if (result.status !== 0) {
    throw new Error(`${label} exited with ${result.status}: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout || "{}");
}

function runBootstrap(rootDir, args) {
  const scriptPath = path.resolve(__dirname, "..", "..", "scripts", "runtime", "bootstrap-run.cjs");
  return spawnSync(process.execPath, [scriptPath, "--root", rootDir, ...args, "--json"], {
    encoding: "utf8",
    cwd: rootDir,
  });
}

function writeAnalysisReport(rootDir) {
  const analysisDir = path.join(rootDir, ".ai", "analysis");
  fs.mkdirSync(analysisDir, { recursive: true });
  fs.writeFileSync(path.join(analysisDir, "ai.report.json"), JSON.stringify({ overall: "READY" }, null, 2), "utf8");
}

const cases = {
  "bootstrap-archives-stopped-run-and-initializes-new-run": () => {
    const rootDir = makeTempRoot("runtime-bootstrap-");
    writeRuntimeState(rootDir, {
      session: {
        run_id: "wf-20260309-001",
        phase: { current: "P3", status: "completed" },
        node: { active_id: "none", state: "idle", owner_persona: "human" },
      },
      phases: {
        "P1": { status: "completed" },
        "P2": { status: "completed" },
        "P3": { status: "completed" },
      },
      nodes: {
        "P1-N1": {
          status: "completed",
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: ["e-1"],
        },
        "P2-N1": {
          status: "completed",
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: ["e-2"],
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });
    writeLedger(rootDir, "# Current Run\n\n- run_id: wf-20260309-001\n");
    writeCapsule(rootDir, {
      capsule_id: "capsule-old-1",
      run_id: "wf-20260309-001",
      node_id: "P2-N3",
      persona: "quality_reviewer",
      summary: "old capsule",
      created_at: "2026-03-09T10:00:00.000Z",
    });
    writeCheckpoint(rootDir, {
      checkpoint_id: "cp-old-1",
      run_id: "wf-20260309-001",
      phase: "P3",
      node_id: "P2-N3",
      session_snapshot_ref: ".ai/workflow/session.yaml",
      task_graph_snapshot_ref: ".ai/workflow/task-graph.yaml",
      journal_offset: 3,
      integrity_hash: "abc123",
      restore_status: "not_restored",
      created_at: "2026-03-09T10:00:00.000Z",
      restore_verified_at: null,
    });
    writeAnalysisReport(rootDir);

    const result = parseJsonOutput(runBootstrap(rootDir, [
      "--task-id", "task-next",
      "--task-title", "Bootstrap next run",
      "--task-mode", "feature",
    ]), "bootstrap-run");

    assertSubset(result, {
      archived_run_id: "wf-20260309-001",
      initial_phase: "P1",
      recommended_next: [{ type: "start_node", target: "P1-N1" }],
    });

    const archiveSessionPath = path.join(rootDir, ".ai", "archive", "wf-20260309-001", "workflow", "session.yaml");
    const archiveReportPath = path.join(rootDir, ".ai", "archive", "wf-20260309-001", "analysis", "ai.report.json");
    if (!fs.existsSync(archiveSessionPath)) {
      throw new Error("expected archived session.yaml");
    }
    if (!fs.existsSync(archiveReportPath)) {
      throw new Error("expected archived ai.report.json");
    }

    const session = readSession(rootDir);
    const graph = readTaskGraph(rootDir);
    const sprintStatus = readSprintStatus(rootDir);
    const journal = readJournalEvents(rootDir);
    const ledger = readLedger(rootDir);

    if (session.run_id === "wf-20260309-001") {
      throw new Error("expected a fresh run_id");
    }
    assertSubset(session, {
      task: { id: "task-next", title: "Bootstrap next run", mode: "feature" },
      phase: { current: "P1", status: "in_progress" },
      node: { active_id: "none", state: "idle", owner_persona: "planner" },
      loop_budget: { consumed_nodes: 0, consumed_failures: 0 },
    });
    assertSubset(sprintStatus, {
      active_phase: "P1",
      recommended_next: [{ type: "start_node", target: "P1-N1" }],
    });
    const p1 = graph.phases.find((item) => item.id === "P1");
    const p2 = graph.phases.find((item) => item.id === "P2");
    const p1n1 = graph.nodes.find((item) => item.id === "P1-N1");
    const p2n1 = graph.nodes.find((item) => item.id === "P2-N1");
    if (!p1 || p1.status !== "in_progress") {
      throw new Error("expected P1 to become in_progress");
    }
    if (!p2 || p2.status !== "pending") {
      throw new Error("expected P2 to become pending");
    }
    if (!p1n1 || p1n1.status !== "ready") {
      throw new Error("expected P1-N1 to become ready");
    }
    if (!p2n1 || p2n1.status !== "pending") {
      throw new Error("expected P2-N1 to become pending");
    }
    if (!journal.some((item) => item.event === "session_started" && item.run_id === session.run_id)) {
      throw new Error("expected fresh session_started event");
    }
    if (!journal.some((item) => item.event === "phase_entered" && item.run_id === session.run_id)) {
      throw new Error("expected fresh phase_entered event");
    }
    if (!ledger || !ledger.includes(session.run_id) || !ledger.includes("Bootstrap next run")) {
      throw new Error("expected bootstrap ledger content");
    }
    if (!ledger.includes("## Active Phase") || !ledger.includes("## Active Node") || !ledger.includes("## Latest Evidence")) {
      throw new Error("expected bootstrap ledger sections from runtime ledger builder");
    }

    const capsulesDir = path.join(rootDir, ".ai", "workflow", "capsules");
    const checkpointsDir = path.join(rootDir, ".ai", "workflow", "checkpoints");
    if (fs.readdirSync(capsulesDir).length !== 0) {
      throw new Error("expected active capsules directory to be empty after bootstrap");
    }
    if (fs.readdirSync(checkpointsDir).length !== 0) {
      throw new Error("expected active checkpoints directory to be empty after bootstrap");
    }
    if (fs.existsSync(path.join(rootDir, ".ai", "analysis", "ai.report.json"))) {
      throw new Error("expected active ai.report.json to be cleared after bootstrap");
    }
  },
  "bootstrap-refuses-active-run": () => {
    const rootDir = makeTempRoot("runtime-bootstrap-block-");
    writeRuntimeState(rootDir, {
      session: {
        run_id: "wf-20260309-002",
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N3", state: "verified", owner_persona: "quality_reviewer" },
      },
    });

    const result = runBootstrap(rootDir, [
      "--task-id", "task-blocked",
      "--task-title", "Should refuse active run",
      "--task-mode", "feature",
    ]);

    if (result.status === 0) {
      throw new Error("expected bootstrap to refuse active run");
    }
    if (!/terminal handoff state|archive\/bootstrap refused/i.test(result.stderr || "")) {
      throw new Error(`expected refusal reason in stderr, got: ${result.stderr}`);
    }

    const session = readSession(rootDir);
    assertSubset(session, {
      run_id: "wf-20260309-002",
      phase: { current: "P2", status: "review" },
      node: { active_id: "P2-N3", owner_persona: "quality_reviewer" },
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
    console.log("BOOTSTRAP_FIXTURES_PASS");
  }
}

main();
