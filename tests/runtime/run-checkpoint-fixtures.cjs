#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { createCapsule } = require("../../scripts/runtime/context-manager.cjs");
const {
  readCapsule,
  readCheckpoint,
  readLedger,
  writeSession,
  writeTaskGraph,
  writeSprintStatus,
  readSession,
  readSprintStatus,
  readTaskGraph,
} = require("../../scripts/runtime/store.cjs");
const {
  appendEvent,
  readEvents,
  getJournalOffset,
  findLatestEvent,
} = require("../../scripts/runtime/journal.cjs");
const {
  createCheckpoint,
  ensurePreDestructiveCheckpoint,
  restoreCheckpoint,
  buildResumeFrontier,
  recoverInterruptedRun,
  listCheckpoints,
} = require("../../scripts/runtime/checkpoints.cjs");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-runtime-checkpoint-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function baseSession() {
  return {
    schema: 4,
    run_id: "wf-20260308-002",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-2", title: "Checkpoint infra", mode: "feature" },
    phase: { current: "execute", status: "in_progress" },
    node: { active_id: "P1-N4", state: "green_pending", owner_persona: "author" },
    loop_budget: {
      max_nodes: 5,
      max_failures: 2,
      max_pending_approvals: 2,
      consumed_nodes: 1,
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
      created_at: "2026-03-08T01:00:00Z",
      updated_at: "2026-03-08T01:00:00Z",
    },
  };
}

function baseTaskGraph() {
  return {
    schema: 4,
    graph_id: "graph-2",
    phases: [
      {
        id: "P1",
        title: "Foundation",
        status: "in_progress",
        depends_on: [],
        entry_condition: ["runtime store complete"],
        exit_gate: { cmd: "node tests/runtime/run-checkpoint-fixtures.cjs", pass_signal: "CHECKPOINT_FIXTURES_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P1-N4"], restore_point: "runtime store stable" },
      },
    ],
    nodes: [
      {
        id: "P1-N4",
        phase_id: "P1",
        title: "Checkpoint infra",
        target: "scripts/runtime/checkpoints.cjs",
        action: "Implement journal/checkpoint/recovery helpers",
        why: "Need durable recovery frontier",
        depends_on: ["P1-N3"],
        verify: { cmd: "node tests/runtime/run-checkpoint-fixtures.cjs", pass_signal: "CHECKPOINT_FIXTURES_PASS" },
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
    active_phase: "P1",
    node_summary: [
      { id: "P1-N4", status: "in_progress", tdd_state: "green_pending" },
    ],
    recommended_next: [
      {
        type: "resume_node",
        target: "P1-N4",
        params: { mode: "verify" },
        reason: "continue checkpoint implementation",
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

const cases = {
  "checkpoint-create-snapshots": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    appendEvent(rootDir, {
      runId: "wf-20260308-002",
      event: "node_started",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "author",
      payload: { step: "checkpointing" },
    });
    const created = createCheckpoint(rootDir, {
      checkpointClass: "node",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "runtime",
      sourceEvent: "verification_recorded",
    });
    assert(created.checkpoint_id, "checkpoint id missing");
    assert(created.integrity_hash, "integrity hash missing");
    assert(fs.existsSync(path.join(rootDir, created.session_snapshot_ref)), "session snapshot missing");
    assert(fs.existsSync(path.join(rootDir, created.task_graph_snapshot_ref)), "task graph snapshot missing");
    const events = readEvents(rootDir);
    assert(events.some((item) => item.event === "checkpoint_created"), "checkpoint_created event missing");
  },
  "restore-frontier-missing": () => {
    const rootDir = makeTempRoot();
    writeSession(rootDir, baseSession());
    writeTaskGraph(rootDir, baseTaskGraph());
    const frontier = buildResumeFrontier(rootDir);
    assert(frontier.recovery_required === true, "missing frontier should require recovery");
    assert(frontier.reasons.includes("resume_frontier_missing"), "missing frontier reason missing");
  },
  "recover-missing-terminal-event": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    appendEvent(rootDir, {
      runId: "wf-20260308-002",
      event: "node_started",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "author",
      payload: { step: "started" },
    });
    const recovery = recoverInterruptedRun(rootDir);
    assert(recovery.recovery_required === true, "recovery should be required");
    assert(recovery.reasons.includes("missing_terminal_event"), "missing terminal reason missing");
    const latest = findLatestEvent(rootDir, "validation_failed");
    assert(latest && latest.payload.terminal_interpretation === "aborted", "aborted interpretation missing");
    const session = readSession(rootDir);
    assert(session.recovery.restore_pending === true, "restore_pending should be true after recovery detection");
  },
  "restore-checkpoint-round-trip": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    appendEvent(rootDir, {
      runId: "wf-20260308-002",
      event: "verification_recorded",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "hook",
      payload: { signal: "green" },
    });
    const created = createCheckpoint(rootDir, {
      checkpointClass: "node",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "runtime",
      sourceEvent: "verification_recorded",
    });
    const changed = baseSession();
    changed.phase.status = "review";
    writeSession(rootDir, changed);
    const restored = restoreCheckpoint(rootDir, created.checkpoint_id, { actor: "runtime" });
    assert(restored.restore_status === "restored", "checkpoint restore status mismatch");
    const session = readSession(rootDir);
    assert(session.phase.status === "in_progress", "session should be restored to checkpoint snapshot");
    const latest = findLatestEvent(rootDir, "checkpoint_restored");
    assert(latest && latest.payload.checkpoint_id === created.checkpoint_id, "checkpoint_restored event missing");
  },
  "resume-frontier-from-sprint-status": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    const frontier = buildResumeFrontier(rootDir);
    assert(frontier.recovery_required === false, "frontier should be resumable");
    assert(frontier.recommended_next[0].target === "P1-N4", "frontier target mismatch");
  },
  "checkpoint-list-order": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    appendEvent(rootDir, {
      runId: "wf-20260308-002",
      event: "verification_recorded",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "hook",
      payload: { signal: "green" },
    });
    const first = createCheckpoint(rootDir, {
      checkpointClass: "node",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "runtime",
      sourceEvent: "verification_recorded",
    });
    const second = createCheckpoint(rootDir, {
      checkpointClass: "review",
      phase: "review",
      nodeId: "P1-N4",
      actor: "runtime",
      sourceEvent: "review_verdict_recorded",
    });
    const checkpoints = listCheckpoints(rootDir);
    assert(checkpoints.length === 2, "checkpoint count mismatch");
    assert(checkpoints[0].checkpoint_id === second.checkpoint_id, "latest checkpoint should be first");
    assert(getJournalOffset(rootDir) >= 3, "journal offset should grow with checkpoint events");
  },
  "pre-destructive-checkpoint-deduplicates-same-target": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);
    const first = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/checkpoints.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });
    const second = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/checkpoints.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });

    assert(first.created === true, "expected first pre-destructive checkpoint to be created");
    assert(second.created === false && second.reason === "existing_pre_destructive_checkpoint", "expected second pre-destructive checkpoint to deduplicate");
    const session = readSession(rootDir);
    const checkpointId = session?.recovery?.last_checkpoint_id;
    const checkpoint = readCheckpoint(rootDir, checkpointId);
    assert(checkpoint && checkpoint.checkpoint_class === "pre_destructive", "expected pre_destructive checkpoint class");
    assert(checkpoint.target_ref === "src/runtime/checkpoints.ts", "expected target_ref metadata");
    assert(checkpoint.operation_kind === "write", "expected operation_kind metadata");
    assert(checkpoint.file_class === "critical_policy_file", "expected file_class metadata");
  },
  "checkpoint-captures-target-capsule-and-git-head": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);

    const gitRef = "0123456789abcdef0123456789abcdef01234567";
    fs.mkdirSync(path.join(rootDir, ".git", "refs", "heads"), { recursive: true });
    fs.writeFileSync(path.join(rootDir, ".git", "HEAD"), "ref: refs/heads/main\n", "utf8");
    fs.writeFileSync(path.join(rootDir, ".git", "refs", "heads", "main"), `${gitRef}\n`, "utf8");

    const targetPath = path.join(rootDir, "src", "runtime", "checkpoints.ts");
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.writeFileSync(targetPath, "export const before = true;\n", "utf8");

    const capsule = createCapsule(rootDir, {
      persona: "author",
      inputSummary: "Checkpoint source capsule",
      outputSummary: "Checkpoint output capsule",
    });
    const ensured = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/checkpoints.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });

    assert(ensured.created === true, "expected checkpoint creation");
    const checkpoint = ensured.checkpoint;
    assert(checkpoint.git_head_ref === gitRef, `expected git_head_ref but got ${JSON.stringify(checkpoint.git_head_ref)}`);
    assert(checkpoint.capsule_id === capsule.capsule_id, `expected capsule_id but got ${JSON.stringify(checkpoint.capsule_id)}`);
    assert(checkpoint.capsule_snapshot_ref, "expected capsule_snapshot_ref");
    assert(checkpoint.target_snapshot_ref, "expected target_snapshot_ref");
    assert(checkpoint.target_snapshot_content_ref, "expected target_snapshot_content_ref");

    const capsuleSnapshotPath = path.join(rootDir, checkpoint.capsule_snapshot_ref);
    const targetSnapshotPath = path.join(rootDir, checkpoint.target_snapshot_ref);
    const targetContentPath = path.join(rootDir, checkpoint.target_snapshot_content_ref);
    assert(fs.existsSync(capsuleSnapshotPath), "expected capsule snapshot file");
    assert(fs.existsSync(targetSnapshotPath), "expected target snapshot metadata file");
    assert(fs.existsSync(targetContentPath), "expected target snapshot content file");

    const capsuleSnapshot = JSON.parse(fs.readFileSync(capsuleSnapshotPath, "utf8"));
    const targetSnapshot = JSON.parse(fs.readFileSync(targetSnapshotPath, "utf8"));
    const targetContent = fs.readFileSync(targetContentPath, "utf8");
    assert(capsuleSnapshot.capsule_id === capsule.capsule_id, "expected latest capsule snapshot");
    assert(targetSnapshot.target_ref === "src/runtime/checkpoints.ts", "expected target snapshot target ref");
    assert(targetSnapshot.exists_before === true, "expected target snapshot exists_before=true");
    assert(targetContent === "export const before = true;\n", "expected target snapshot content");
  },
  "restore-pre-destructive-checkpoint-restores-target-content": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);

    const targetPath = path.join(rootDir, "src", "runtime", "checkpoints.ts");
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.writeFileSync(targetPath, "export const before = true;\n", "utf8");

    const ensured = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/checkpoints.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });
    fs.writeFileSync(targetPath, "export const before = false;\n", "utf8");

    const restored = restoreCheckpoint(rootDir, ensured.checkpoint.checkpoint_id, {
      actor: "runtime",
      restoreTargetSnapshot: true,
    });
    const targetContent = fs.readFileSync(targetPath, "utf8");
    assert(restored.restore_status === "restored", "expected restored checkpoint status");
    assert(targetContent === "export const before = true;\n", `expected restored file content but got ${JSON.stringify(targetContent)}`);
  },
  "restore-pre-destructive-checkpoint-removes-new-file": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);

    const targetPath = path.join(rootDir, "src", "runtime", "new-file.ts");
    const ensured = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/new-file.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.writeFileSync(targetPath, "export const createdLater = true;\n", "utf8");

    const restored = restoreCheckpoint(rootDir, ensured.checkpoint.checkpoint_id, {
      actor: "runtime",
      restoreTargetSnapshot: true,
    });

    assert(restored.restore_status === "restored", "expected restored checkpoint status for newly created file");
    assert(fs.existsSync(targetPath) === false, "expected restore to remove file that did not exist before checkpoint");
  },
  "pre-destructive-checkpoint-without-git-keeps-null-head-ref": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);

    const targetPath = path.join(rootDir, "src", "runtime", "checkpoints.ts");
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.writeFileSync(targetPath, "export const before = true;\n", "utf8");

    const ensured = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/checkpoints.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });

    assert(ensured.checkpoint.git_head_ref === null, `expected null git_head_ref but got ${JSON.stringify(ensured.checkpoint.git_head_ref)}`);
  },
  "large-target-snapshot-requires-manual-restore": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);

    const targetPath = path.join(rootDir, "src", "runtime", "large-file.ts");
    fs.mkdirSync(path.dirname(targetPath), { recursive: true });
    fs.writeFileSync(targetPath, `${"x".repeat(300 * 1024)}\n`, "utf8");

    const ensured = ensurePreDestructiveCheckpoint(rootDir, {
      actor: "hook",
      phase: "execute",
      nodeId: "P1-N4",
      targetRef: "src/runtime/large-file.ts",
      operationKind: "write",
      fileClass: "critical_policy_file",
      sourceEvent: "pre_destructive_guard",
    });

    const checkpointBeforeRestore = readCheckpoint(rootDir, ensured.checkpoint.checkpoint_id);
    assert(checkpointBeforeRestore.target_snapshot_content_ref === null, "expected large file snapshot to skip inline content capture");

    fs.writeFileSync(targetPath, "export const drifted = true;\n", "utf8");
    const restored = restoreCheckpoint(rootDir, ensured.checkpoint.checkpoint_id, {
      actor: "runtime",
      restoreTargetSnapshot: true,
    });

    const session = readSession(rootDir);
    const latestFailure = findLatestEvent(rootDir, "validation_failed");
    assert(restored.restore_status === "restore_failed", "expected restore_failed when target snapshot needs manual restore");
    assert(session.recovery.restore_pending === true, "expected restore_pending after manual restore requirement");
    assert(session.recovery.restore_reason === "target_snapshot_requires_manual_restore", `unexpected restore_reason ${JSON.stringify(session.recovery.restore_reason)}`);
    assert(latestFailure && latestFailure.payload?.reason === "target_snapshot_requires_manual_restore", "expected validation_failed event for manual restore requirement");
  },
  "restore-node-checkpoint-restores-sprint-status-capsule-and-ledger": () => {
    const rootDir = makeTempRoot();
    seedRuntime(rootDir);

    const capsule = createCapsule(rootDir, {
      persona: "author",
      inputSummary: "Runtime checkpoint input",
      outputSummary: "Runtime checkpoint output",
    });
    appendEvent(rootDir, {
      runId: "wf-20260308-002",
      event: "verification_recorded",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "hook",
      payload: { signal: "green" },
    });
    const checkpoint = createCheckpoint(rootDir, {
      checkpointClass: "node",
      phase: "execute",
      nodeId: "P1-N4",
      actor: "runtime",
      sourceEvent: "verification_recorded",
    });

    writeSprintStatus(rootDir, {
      schema: 4,
      active_phase: "P1",
      node_summary: [{ id: "P1-N4", status: "blocked", tdd_state: "green_pending" }],
      recommended_next: [{
        type: "human_intervention",
        target: "P1-N4",
        params: {},
        reason: "drifted frontier",
        blocking_on: [],
        priority: "now",
      }],
    });
    const capsulePath = path.join(rootDir, ".ai", "workflow", "capsules", `${capsule.capsule_id}.json`);
    if (fs.existsSync(capsulePath)) {
      fs.rmSync(capsulePath, { force: true });
    }

    const restored = restoreCheckpoint(rootDir, checkpoint.checkpoint_id, {
      actor: "runtime",
    });
    const sprintStatus = readSprintStatus(rootDir);
    const restoredCapsule = readCapsule(rootDir, capsule.capsule_id);
    const ledger = readLedger(rootDir);

    assert(restored.restore_status === "restored", "expected restored node checkpoint status");
    assert(Array.isArray(sprintStatus.recommended_next) && sprintStatus.recommended_next[0]?.target === "P1-N4", "expected sprint-status snapshot restored");
    assert(restoredCapsule && restoredCapsule.capsule_id === capsule.capsule_id, "expected capsule snapshot restored into active capsules");
    assert(ledger && ledger.includes("## Active Phase") && ledger.includes("## Recommended Next"), "expected ledger to be refreshed after restore");
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
    }
  }
  if (failed) {
    process.exit(1);
  }
  console.log("CHECKPOINT_FIXTURES_PASS");
}

main();
