"use strict";

const {
  buildFixtureState,
  assertSubset,
  copyRuntimeFixtureFiles,
  makeTempRoot,
} = require("./runtime-fixture-lib.cjs");
const {
  appendJournalEvents,
  readCheckpoint,
  readJournalEvents,
  readLedger,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("../../scripts/runtime/store.cjs");
const { applyRuntimeTransition } = require("../../scripts/runtime/transition-applier.cjs");

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  if (Array.isArray(state.journal) && state.journal.length > 0) {
    appendJournalEvents(rootDir, state.journal);
  }
  return state;
}

const cases = {
  "start-node-transition-updates-all-assets": () => {
    const rootDir = makeTempRoot("transition-start-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_phase: "P2",
        active_node: "P2-N2",
        next_persona: "author",
        next_capability: "code_edit",
        recommended_next: [{ type: "start_node", target: "P2-N2", params: {}, reason: "start", blocking_on: [], priority: "now" }],
        emit_events: ["node_started"],
      },
    });

    assertSubset(result, {
      filesUpdated: [".ai/workflow/session.yaml", ".ai/workflow/task-graph.yaml", ".ai/workflow/sprint-status.yaml", ".ai/workflow/journal.jsonl"],
      session: { node: { active_id: "P2-N2", owner_persona: "author" } },
      sprintStatus: { active_phase: "P2", recommended_next: [{ type: "start_node", target: "P2-N2" }] },
    });

    const session = readSession(rootDir);
    const taskGraph = readTaskGraph(rootDir);
    const sprintStatus = readSprintStatus(rootDir);
    const ledger = readLedger(rootDir);
    const journal = readJournalEvents(rootDir);

    assertSubset(session, { node: { active_id: "P2-N2", owner_persona: "author" } });
    assertSubset(sprintStatus, { recommended_next: [{ type: "start_node", target: "P2-N2" }] });
    const node = taskGraph.nodes.find((item) => item.id === "P2-N2");
    if (!node || node.status !== "in_progress") {
      throw new Error("expected P2-N2 status=in_progress");
    }
    if (!journal.some((item) => item.event === "session_started")) {
      throw new Error("expected session_started event");
    }
    if (!journal.some((item) => item.event === "node_started")) {
      throw new Error("expected node_started event");
    }
    if (!ledger || !ledger.includes("## Active Node") || !ledger.includes("start_node:P2-N2")) {
      throw new Error("expected ledger to be refreshed after start-node transition");
    }
  },
  "phase-transition-completes-current-phase-and-enters-next": () => {
    const rootDir = makeTempRoot("transition-phase-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_phase: "P3",
        active_node: "none",
        next_persona: "planner",
        next_capability: "planning",
        recommended_next: [{ type: "enter_phase", target: "P3", params: {}, reason: "advance", blocking_on: [], priority: "now" }],
        emit_events: ["phase_completed", "phase_entered"],
      },
    });

    assertSubset(result, {
      session: { phase: { current: "P3", status: "in_progress" }, node: { active_id: "none", owner_persona: "planner" } },
      sprintStatus: { active_phase: "P3", recommended_next: [{ type: "enter_phase", target: "P3" }] },
    });

    const taskGraph = readTaskGraph(rootDir);
    const prevPhase = taskGraph.phases.find((item) => item.id === "P2");
    const nextPhase = taskGraph.phases.find((item) => item.id === "P3");
    if (!prevPhase || prevPhase.status !== "completed") {
      throw new Error("expected P2 to become completed");
    }
    if (!nextPhase || nextPhase.status !== "in_progress") {
      throw new Error("expected P3 to become in_progress");
    }
    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "phase_completed")) {
      throw new Error("expected phase_completed event");
    }
    if (!journal.some((item) => item.event === "phase_entered")) {
      throw new Error("expected phase_entered event");
    }
  },
  "resume-transition-clears-restore-pending": () => {
    const rootDir = makeTempRoot("transition-resume-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        node: { active_id: "P2-N1", state: "red_pending", owner_persona: "author" },
        recovery: { restore_pending: true, restore_reason: "missing_terminal_event" },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "resume",
      decision: {
        route_verdict: "resume",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "author",
        next_capability: "code_edit",
        block_reason: "restore_pending",
        recommended_next: [{ type: "resume_node", target: "P2-N1", params: {}, reason: "resume", blocking_on: [], priority: "now" }],
        emit_events: ["session_resumed"],
      },
    });

    assertSubset(result, {
      session: { recovery: { restore_pending: false, restore_reason: null }, node: { active_id: "P2-N1" } },
      sprintStatus: { recommended_next: [{ type: "resume_node", target: "P2-N1" }] },
    });
    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "session_resumed")) {
      throw new Error("expected session_resumed event");
    }
  },
  "review-complete-finalizes-node-before-starting-next": () => {
    const rootDir = makeTempRoot("transition-review-complete-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_state: "green_verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "ready",
          tdd_required: false,
          tdd_state: "not_applicable",
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_phase: "P2",
        active_node: "P2-N2",
        next_persona: "author",
        next_capability: "code_edit",
        policy_verdict: {
          completion: { node_complete_ready: true },
        },
        recommended_next: [{ type: "start_node", target: "P2-N2", params: {}, reason: "continue", blocking_on: [], priority: "now" }],
        emit_events: ["node_completed", "node_started"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N2", owner_persona: "author" },
        loop_budget: { consumed_nodes: 1 },
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const completedNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    const startedNode = taskGraph.nodes.find((item) => item.id === "P2-N2");
    if (!completedNode || completedNode.status !== "completed") {
      throw new Error("expected P2-N1 to become completed");
    }
    if (!startedNode || startedNode.status !== "in_progress") {
      throw new Error("expected P2-N2 to become in_progress");
    }
    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "node_completed" && item.node_id === "P2-N1")) {
      throw new Error("expected node_completed event for P2-N1");
    }
    if (!journal.some((item) => item.event === "node_started" && item.node_id === "P2-N2")) {
      throw new Error("expected node_started event for P2-N2");
    }
    if (!journal.some((item) => item.event === "checkpoint_created" && item.node_id === "P2-N1")) {
      throw new Error("expected checkpoint_created event for completed node");
    }
    const updatedSession = readSession(rootDir);
    const checkpointId = updatedSession?.recovery?.last_checkpoint_id;
    if (!checkpointId) {
      throw new Error("expected last_checkpoint_id after node completion");
    }
    const checkpoint = readCheckpoint(rootDir, checkpointId);
    if (!checkpoint || checkpoint.node_id !== "P2-N1" || checkpoint.restore_source_event !== "node_completed") {
      throw new Error("expected node checkpoint snapshot after completion");
    }
  },
  "request-approval-transition-materializes-active-request": () => {
    const rootDir = makeTempRoot("transition-request-approval-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "author" },
      },
      nodes: {
        "P2-N1": {
          status: "in_progress",
          tdd_state: "green_pending",
          approval_ref: null,
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: {
        route_verdict: "block",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "human",
        next_capability: "human_approval",
        recommended_next: [{
          type: "request_approval",
          target: "P2-N1",
          params: { risk_class: "critical", approval_mode: "manual_required", grant_scope: "once" },
          reason: "approval required",
          blocking_on: [],
          priority: "now",
        }],
        emit_events: ["approval_requested"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "once",
          last_approval_mode: "manual_required",
          active_request: {
            action: "human_approval",
            target_ref: "P2-N1",
            risk_class: "critical",
            approval_mode: "manual_required",
            grant_scope: "once",
            status: "pending",
          },
        },
      },
      sprintStatus: {
        recommended_next: [{ type: "request_approval", target: "P2-N1" }],
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const gatedNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!gatedNode || gatedNode.status !== "blocked") {
      throw new Error("expected P2-N1 to become blocked while approval is pending");
    }
    if (!gatedNode.approval_ref) {
      throw new Error("expected approval_ref to be written to the gated node");
    }

    const session = readSession(rootDir);
    if (session.approvals.active_request.approval_id !== gatedNode.approval_ref) {
      throw new Error("expected session active_request.approval_id to match node approval_ref");
    }

    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "approval_requested" && item.node_id === "P2-N1")) {
      throw new Error("expected approval_requested event for P2-N1");
    }
  },
  "retry-transition-restarts-failed-node": () => {
    const rootDir = makeTempRoot("transition-retry-node-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "human" },
      },
      nodes: {
        "P2-N1": {
          status: "failed",
          tdd_state: "green_pending",
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: {
        route_verdict: "resume",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "author",
        next_capability: "code_edit",
        recommended_next: [{
          type: "retry_node",
          target: "P2-N1",
          params: { next_attempt: 2 },
          reason: "retry failed node",
          blocking_on: [],
          priority: "now",
        }],
        emit_events: ["node_started"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N1", owner_persona: "author" },
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const retriedNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!retriedNode || retriedNode.status !== "in_progress") {
      throw new Error("expected P2-N1 to return to in_progress during retry");
    }
    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "node_started" && item.node_id === "P2-N1")) {
      throw new Error("expected node_started event for retry transition");
    }
  },
  "hold-retry-backoff-keeps-node-failed": () => {
    const rootDir = makeTempRoot("transition-retry-backoff-hold-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "human" },
      },
      nodes: {
        "P2-N1": {
          status: "failed",
          tdd_state: "green_pending",
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "resume",
      decision: {
        route_verdict: "hold",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "author",
        next_capability: "code_edit",
        block_reason: "retry_backoff_pending",
        recommended_next: [{
          type: "resume_node",
          target: "P2-N1",
          params: { wait_until: "2026-03-09T10:10:00.000Z" },
          reason: "wait until retry backoff window opens",
          blocking_on: [],
          priority: "now",
        }],
        emit_events: [],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "author" },
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!node || node.status !== "failed") {
      throw new Error("expected failed node to remain failed while waiting for retry backoff");
    }
  },
  "failed-request-approval-persists-transition-context": () => {
    const rootDir = makeTempRoot("transition-node-failed-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "author" },
      },
      nodes: {
        "P2-N1": {
          status: "in_progress",
          tdd_state: "green_pending",
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      actionContext: {
        retryContext: {
          attemptsUsed: 1,
          failureKind: "contract_mismatch",
          source: "hook",
        },
      },
      decision: {
        route_verdict: "block",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "human",
        next_capability: "human_approval",
        policy_verdict: {
          route_effect: "require_human",
          primary_reason: "approval_required",
        },
        recommended_next: [{
          type: "request_approval",
          target: "P2-N1",
          params: {
            risk_class: "high",
            approval_mode: "manual_required",
            grant_scope: "once",
          },
          reason: "approval required",
          blocking_on: [],
          priority: "now",
        }],
        emit_events: ["node_failed", "approval_requested"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "human" },
        loop_budget: { consumed_failures: 1 },
        approvals: {
          pending: true,
          pending_count: 1,
        },
      },
    });

    const journal = readJournalEvents(rootDir);
    const failureEvent = journal.find((item) => item.event === "node_failed" && item.node_id === "P2-N1");
    if (!failureEvent) {
      throw new Error("expected node_failed event");
    }
    assertSubset(failureEvent, {
      payload: {
        transition_context: {
          failure_kind: "contract_mismatch",
        },
      },
    });
    if (!journal.some((item) => item.event === "approval_requested" && item.node_id === "P2-N1")) {
      throw new Error("expected approval_requested event for failed node");
    }
  },
  "timeout-request-approval-increments-failure-budget": () => {
    const rootDir = makeTempRoot("transition-timeout-approval-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "author" },
      },
      nodes: {
        "P2-N1": {
          status: "in_progress",
          tdd_state: "green_pending",
          timeout_policy: {
            timeout_seconds: 120,
            grace_seconds: 10,
            on_timeout: "require_human",
          },
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: {
        route_verdict: "block",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "human",
        next_capability: "human_approval",
        recommended_next: [{
          type: "request_approval",
          target: "P2-N1",
          params: { risk_class: "high", approval_mode: "manual_required", grant_scope: "once" },
          reason: "node_timed_out",
          blocking_on: [],
          priority: "now",
        }],
        emit_events: ["node_timed_out", "approval_requested"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "human" },
        loop_budget: { consumed_failures: 1 },
        approvals: {
          pending: true,
          pending_count: 1,
        },
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const timedOutNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!timedOutNode || timedOutNode.status !== "failed") {
      throw new Error("expected timed out node to be marked failed");
    }
    const journal = readJournalEvents(rootDir);
    const timeoutEvent = journal.find((item) => item.event === "node_timed_out" && item.node_id === "P2-N1");
    if (!timeoutEvent) {
      throw new Error("expected node_timed_out event");
    }
    assertSubset(timeoutEvent, {
      payload: {
        transition_context: {
          failure_kind: "timeout",
          timeout_triggered: true,
        },
      },
    });
    if (!journal.some((item) => item.event === "approval_requested" && item.node_id === "P2-N1")) {
      throw new Error("expected approval_requested event for timed out node");
    }
  },
  "verify-terminal-handoff-completes-session": () => {
    const rootDir = makeTempRoot("transition-terminal-handoff-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      phases: {
        "P3": { status: "completed" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_state: "green_verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
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

    const result = applyRuntimeTransition(rootDir, {
      mode: "verify",
      decision: {
        route_verdict: "hold",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "human",
        next_capability: "human_approval",
        policy_verdict: {
          completion: {
            node_complete_ready: true,
            phase_complete_ready: true,
            stop_gate_ready: true,
          },
        },
        recommended_next: [{ type: "human_intervention", target: "session", params: {}, reason: "session complete; handoff to human", blocking_on: [], priority: "now" }],
        emit_events: ["node_completed", "phase_completed", "session_stopped"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "completed" },
        node: { active_id: "none", state: "idle", owner_persona: "human" },
      },
      sprintStatus: {
        active_phase: "P2",
        recommended_next: [{ type: "human_intervention", target: "session" }],
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const activePhase = taskGraph.phases.find((item) => item.id === "P2");
    const finalNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!activePhase || activePhase.status !== "completed") {
      throw new Error("expected P2 to become completed during terminal handoff");
    }
    if (!finalNode || finalNode.status !== "completed") {
      throw new Error("expected P2-N1 to become completed during terminal handoff");
    }

    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "session_stopped" && item.phase === "P2")) {
      throw new Error("expected session_stopped event");
    }
    const ledger = readLedger(rootDir);
    if (!ledger || !ledger.includes("## Pending Approval") || !ledger.includes("human_intervention:session")) {
      throw new Error("expected ledger to reflect terminal handoff state");
    }
  },
  "verify-auto-advance-starts-next-ready-node": () => {
    const rootDir = makeTempRoot("transition-verify-auto-start-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_state: "green_verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "ready",
          tdd_required: false,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pending", quality_review: "pending" },
        },
      },
    });

    const result = applyRuntimeTransition(rootDir, {
      mode: "verify",
      allowVerifyTransition: true,
      decision: {
        route_verdict: "advance",
        active_phase: "P2",
        active_node: "P2-N2",
        next_persona: "author",
        next_capability: "code_edit",
        policy_verdict: {
          completion: {
            node_complete_ready: true,
            phase_complete_ready: false,
            stop_gate_ready: false,
          },
        },
        recommended_next: [{ type: "start_node", target: "P2-N2", params: {}, reason: "start highest priority ready node", blocking_on: [], priority: "now" }],
        emit_events: ["node_completed", "node_started"],
      },
    });

    assertSubset(result, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N2", owner_persona: "author" },
        loop_budget: { consumed_nodes: 1 },
      },
      sprintStatus: {
        active_phase: "P2",
        recommended_next: [{ type: "start_node", target: "P2-N2" }],
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const completedNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    const startedNode = taskGraph.nodes.find((item) => item.id === "P2-N2");
    if (!completedNode || completedNode.status !== "completed") {
      throw new Error("expected P2-N1 to become completed during verify auto advance");
    }
    if (!startedNode || startedNode.status !== "in_progress") {
      throw new Error("expected P2-N2 to become in_progress during verify auto advance");
    }
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
    console.log("TRANSITION_FIXTURES_PASS");
  }
}

main();
