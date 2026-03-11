#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { appendEvent } = require("../../scripts/runtime/journal.cjs");
const { buildReviewHandoffCapsule, compactContext, createCapsule, getLatestCapsule } = require("../../scripts/runtime/context-manager.cjs");
const { listCapsules, readCapsule, writeSession, writeSprintStatus, writeTaskGraph } = require("../../scripts/runtime/store.cjs");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-runtime-context-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function baseSession() {
  return {
    schema: 4,
    run_id: "wf-20260309-002",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-p5-context", title: "Context continuity", mode: "feature" },
    phase: { current: "P5", status: "in_progress" },
    node: { active_id: "P5-N3", state: "green_pending", owner_persona: "author" },
    loop_budget: {
      max_nodes: 8,
      max_failures: 2,
      max_pending_approvals: 1,
      consumed_nodes: 3,
      consumed_failures: 0,
    },
    context_budget: {
      strategy: "hybrid",
      capsule_refresh_threshold: 4,
      summary_required_after_turns: 8,
    },
    workspace: { root: "D:/repo/demo", sandbox_mode: "workspace_write" },
    approvals: {
      pending: false,
      pending_count: 0,
      last_grant_scope: "none",
      last_approval_mode: "none",
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "node-123",
      restore_pending: false,
      restore_reason: null,
    },
    timestamps: {
      created_at: "2026-03-09T02:00:00Z",
      updated_at: "2026-03-09T02:00:00Z",
    },
  };
}

function baseTaskGraph() {
  return {
    schema: 4,
    graph_id: "graph-p5-context",
    phases: [
      {
        id: "P5",
        title: "Context continuity",
        status: "in_progress",
        depends_on: ["P4"],
        entry_condition: ["P4 completed"],
        exit_gate: { cmd: "node tests/runtime/run-context-fixtures.cjs", pass_signal: "CONTEXT_CONTINUITY_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P5-N3"], restore_point: "P5-N2 stable" },
      },
    ],
    nodes: [
      {
        id: "P5-N3",
        phase_id: "P5",
        title: "Context manager",
        target: "scripts/runtime/context-manager.cjs",
        action: "Manage capsules, compaction, and review handoff",
        why: "Need context continuity for long sessions",
        depends_on: ["P5-N2"],
        verify: { cmd: "node tests/runtime/run-context-fixtures.cjs", pass_signal: "CONTEXT_CONTINUITY_PASS" },
        risk_level: "high",
        tdd_required: true,
        status: "in_progress",
        tdd_state: "green_pending",
        owner_persona: "author",
        review_state: { spec_review: "pending", quality_review: "pending" },
        evidence_refs: ["verify-staging.json"],
        output_refs: ["implementation-notes.md"],
        approval_ref: null,
        capability: "code_edit",
        priority: "high",
      },
    ],
  };
}

function baseSprintStatus() {
  return {
    schema: 4,
    active_phase: "P5",
    node_summary: [
      { id: "P5-N3", status: "in_progress", tdd_state: "green_pending" },
    ],
    recommended_next: [
      {
        type: "resume_node",
        target: "P5-N3",
        params: { mode: "verify" },
        reason: "continue context manager implementation",
        blocking_on: [],
        priority: "now",
      },
    ],
  };
}

function seedRuntime(rootDir) {
  writeSession(rootDir, baseSession());
  writeTaskGraph(rootDir, baseTaskGraph());
  writeSprintStatus(rootDir, baseSprintStatus());
  appendEvent(rootDir, {
    runId: "wf-20260309-002",
    event: "node_started",
    phase: "P5",
    nodeId: "P5-N3",
    actor: "author",
    payload: { step: "start" },
  });
  appendEvent(rootDir, {
    runId: "wf-20260309-002",
    event: "verification_recorded",
    phase: "P5",
    nodeId: "P5-N3",
    actor: "hook",
    payload: { signal: "green" },
  });
}

const cases = {
  "resume-frontier-lost-after-compaction": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    writeSprintStatus(rootDir, {
      schema: 4,
      active_phase: "P5",
      node_summary: [{ id: "P5-N3", status: "in_progress", tdd_state: "green_pending" }],
      recommended_next: [],
    });

    let failed = false;
    try {
      compactContext(rootDir, {
        contextUtilization: 0.85,
        turnsSinceSummary: 9,
      });
    } catch (error) {
      failed = true;
      assert(/CONTEXT_MANAGER_RESUME_FRONTIER_LOST/i.test(String(error.message || "")), `expected lost frontier failure but got ${JSON.stringify(error.message)}`);
    }
    assert(failed === true, "expected compaction to fail when resume frontier is missing");
  },
  "compact-context-preserves-frontier": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    const result = compactContext(rootDir, {
      contextUtilization: 0.84,
      turnsSinceSummary: 9,
      turnsSinceCapsule: 4,
    });

    assert(result.compacted === true, "compaction should run when thresholds are exceeded");
    assert(result.reasons.includes("context_utilization_high"), "expected context utilization reason");
    assert(result.active_capsule.persona === "author", "active capsule should target author persona");
    assert(result.resume_frontier.recommended_next[0].target === "P5-N3", "resume frontier target mismatch");
    assert(result.hot_context.capsule_id === result.active_capsule.capsule_id, "hot context should point to active capsule");

    const capsules = listCapsules(rootDir);
    assert(capsules.length >= 1, "expected at least one capsule file");
    const saved = readCapsule(rootDir, result.active_capsule.capsule_id);
    assert(saved && saved.output_summary.includes("Resume frontier"), "saved capsule should preserve output summary");
    const latest = getLatestCapsule(rootDir);
    assert(latest && latest.capsule_id === result.active_capsule.capsule_id, "latest capsule should match active capsule");
  },
  "review-handoff-capsule": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    const authorCapsule = createCapsule(rootDir, {
      persona: "author",
      inputSummary: "Author implementation summary",
      outputSummary: "Author handoff ready",
      verdict: "pending",
    });
    const reviewerCapsule = buildReviewHandoffCapsule(rootDir, {
      sourceCapsule: authorCapsule,
    });

    assert(reviewerCapsule.persona === "spec_reviewer", `expected spec_reviewer but got ${JSON.stringify(reviewerCapsule.persona)}`);
    assert(reviewerCapsule.verdict === "pending", "review handoff capsule should be pending");
    assert(reviewerCapsule.constraints.includes("review_isolation"), "review handoff should enforce review isolation");
    assert(reviewerCapsule.input_summary.includes("Author handoff ready"), "review handoff should carry source capsule summary");
    assert(Array.isArray(reviewerCapsule.evidence_refs) && reviewerCapsule.evidence_refs.includes("verify-staging.json"), "review handoff should preserve evidence refs");
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
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }

  let failed = false;
  for (const [caseName, run] of selected) {
    try {
      run();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
      break;
    }
  }

  if (failed) {
    process.exit(1);
  }

  console.log("CONTEXT_CONTINUITY_PASS");
}

main();
