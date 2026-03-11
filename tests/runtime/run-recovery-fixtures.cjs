#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { appendEvent, findLatestEvent } = require("../../scripts/runtime/journal.cjs");
const { createCheckpoint } = require("../../scripts/runtime/checkpoints.cjs");
const { bridgeGeminiRestore } = require("../../scripts/runtime/recovery-bridge.cjs");
const { readCheckpoint, readSession, readSprintStatus, writeSession, writeSprintStatus, writeTaskGraph } = require("../../scripts/runtime/store.cjs");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-runtime-recovery-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function baseSession() {
  return {
    schema: 4,
    run_id: "wf-20260309-001",
    engine: { kind: "gemini_cli", adapter_version: 1 },
    task: { id: "task-p5", title: "Recovery bridge", mode: "feature" },
    phase: { current: "execute", status: "in_progress" },
    node: { active_id: "P5-N2", state: "green_pending", owner_persona: "author" },
    loop_budget: {
      max_nodes: 8,
      max_failures: 2,
      max_pending_approvals: 1,
      consumed_nodes: 2,
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
      last_checkpoint_id: null,
      restore_pending: false,
      restore_reason: null,
    },
    timestamps: {
      created_at: "2026-03-09T01:00:00Z",
      updated_at: "2026-03-09T01:00:00Z",
    },
  };
}

function baseTaskGraph() {
  return {
    schema: 4,
    graph_id: "graph-p5",
    phases: [
      {
        id: "P5",
        title: "Recovery",
        status: "in_progress",
        depends_on: ["P4"],
        entry_condition: ["P4 completed"],
        exit_gate: { cmd: "node tests/runtime/run-recovery-fixtures.cjs", pass_signal: "RECOVERY_FIXTURES_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P5-N1", "P5-N2"], restore_point: "P4 stable" },
      },
    ],
    nodes: [
      {
        id: "P5-N2",
        phase_id: "P5",
        title: "Recovery bridge",
        target: "scripts/runtime/recovery-bridge.cjs",
        action: "Bridge Gemini restore semantics to V4 recovery state",
        why: "Need durable restore semantics and replay metadata",
        depends_on: ["P5-N1"],
        verify: { cmd: "node tests/runtime/run-recovery-fixtures.cjs", pass_signal: "RECOVERY_FIXTURES_PASS" },
        risk_level: "high",
        tdd_required: true,
        status: "in_progress",
        tdd_state: "green_pending",
        owner_persona: "author",
        review_state: { spec_review: "pending", quality_review: "pending" },
        evidence_refs: [],
        output_refs: [],
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
      { id: "P5-N2", status: "in_progress", tdd_state: "green_pending" },
    ],
    recommended_next: [
      {
        type: "resume_node",
        target: "P5-N2",
        params: { mode: "verify" },
        reason: "continue recovery bridge implementation",
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
}

function createNodeCheckpoint(rootDir) {
  appendEvent(rootDir, {
    runId: "wf-20260309-001",
    event: "verification_recorded",
    phase: "execute",
    nodeId: "P5-N2",
    actor: "hook",
    payload: { signal: "green" },
  });
  return createCheckpoint(rootDir, {
    checkpointClass: "node",
    phase: "execute",
    nodeId: "P5-N2",
    actor: "runtime",
    sourceEvent: "verification_recorded",
  });
}

const cases = {
  "restore-missing-tool-call-metadata": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    const checkpoint = createNodeCheckpoint(rootDir);
    const checkpointFile = path.join(rootDir, ".tmp", "gemini-checkpoint-invalid.json");
    writeJson(checkpointFile, { history: [] });

    let failed = false;
    try {
      bridgeGeminiRestore(rootDir, {
        checkpointId: checkpoint.checkpoint_id,
        geminiCheckpointPath: checkpointFile,
        actor: "adapter",
      });
    } catch (error) {
      failed = true;
      assert(/missing_tool_call_metadata/i.test(String(error.message || "")), `expected toolCall validation failure but got ${JSON.stringify(error.message)}`);
    }

    assert(failed === true, "expected bridge to fail when Gemini checkpoint lacks toolCall metadata");
    const session = readSession(rootDir);
    assert(session.recovery.restore_pending === true, "restore_pending should be true after invalid Gemini checkpoint");
    assert(session.recovery.restore_reason === "missing_tool_call_metadata", `unexpected restore_reason ${JSON.stringify(session.recovery.restore_reason)}`);
    const latest = findLatestEvent(rootDir, "validation_failed");
    assert(latest && latest.payload.reason === "missing_tool_call_metadata", "validation_failed event should record missing toolCall metadata");
  },
  "gemini-restore-bridge-round-trip": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    const checkpoint = createNodeCheckpoint(rootDir);

    const driftedSession = baseSession();
    driftedSession.phase.status = "review";
    driftedSession.recovery.restore_pending = true;
    driftedSession.recovery.restore_reason = "manual_restore_requested";
    writeSession(rootDir, driftedSession);
    writeSprintStatus(rootDir, {
      schema: 4,
      active_phase: "P5",
      node_summary: [{ id: "P5-N2", status: "blocked", tdd_state: "green_pending" }],
      recommended_next: [],
    });

    const checkpointFile = path.join(rootDir, ".tmp", "gemini-checkpoint-valid.json");
    writeJson(checkpointFile, {
      history: [
        { role: "user", parts: [{ text: "continue restore" }] },
      ],
      toolCall: {
        toolName: "write_file",
        toolArgs: {
          file_path: "src/app.ts",
          content: "export const restored = true;\n",
        },
      },
    });

    const result = bridgeGeminiRestore(rootDir, {
      checkpointId: checkpoint.checkpoint_id,
      geminiCheckpointPath: checkpointFile,
      actor: "adapter",
    });

    assert(result.checkpoint_id === checkpoint.checkpoint_id, "checkpoint id mismatch");
    assert(result.tool_replay.tool_name === "write_file", "tool replay name mismatch");
    assert(result.resume_frontier.recovery_required === false, "resume frontier should be resumable after bridge restore");
    assert(result.resume_frontier.recommended_next[0].target === "P5-N2", "resume frontier target mismatch");

    const restoredCheckpoint = readCheckpoint(rootDir, checkpoint.checkpoint_id);
    assert(restoredCheckpoint.restore_status === "restored", "checkpoint should be marked restored");

    const session = readSession(rootDir);
    assert(session.phase.status === "in_progress", "session snapshot should be restored from checkpoint");
    assert(session.recovery.restore_pending === false, "restore_pending should be cleared after successful restore");
    assert(session.recovery.last_checkpoint_id === checkpoint.checkpoint_id, "last checkpoint id should be preserved");

    const sprintStatus = readSprintStatus(rootDir);
    assert(Array.isArray(sprintStatus.recommended_next) && sprintStatus.recommended_next.length === 1, "sprint status should get a restored recommended_next");
    assert(sprintStatus.recommended_next[0].params.resume_mode === "gemini_restore", "resume mode should indicate gemini_restore");

    const resumed = findLatestEvent(rootDir, "session_resumed");
    assert(resumed && resumed.payload.source === "gemini_restore", "session_resumed should record gemini_restore source");
    assert(resumed.payload.tool_name === "write_file", "session_resumed should record tool name");
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

  console.log("RECOVERY_FIXTURES_PASS");
}

main();
