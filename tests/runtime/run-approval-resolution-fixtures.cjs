#!/usr/bin/env node
"use strict";

const {
  buildFixtureState,
  assertSubset,
  copyRuntimeFixtureFiles,
  makeTempRoot,
} = require("./runtime-fixture-lib.cjs");
const {
  readJournalEvents,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("../../scripts/runtime/store.cjs");
const { applyApprovalDecision } = require("../../scripts/runtime/approval-resolution.cjs");

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  return state;
}

const cases = {
  "approve-pending-request-clears-queue-and-refreshes-next": () => {
    const rootDir = makeTempRoot("approval-resolve-approve-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "none",
          last_approval_mode: "manual_required",
          active_request: {
            approval_id: "apr-001",
            action: "write_file",
            target_ref: "P2-N1",
            risk_class: "critical",
            approval_mode: "manual_required",
            grant_scope: "once",
            status: "pending",
            requested_at: "2026-03-09T18:00:00Z",
          },
          grants: [],
        },
      },
      nodes: {
        "P2-N1": {
          status: "blocked",
          tdd_state: "green_pending",
          approval_ref: "apr-001",
        },
      },
      sprint_status: {
        recommended_next: [{ type: "request_approval", target: "P2-N1" }],
      },
    });

    const result = applyApprovalDecision(rootDir, {
      decision: "approved",
      approvalId: "apr-001",
      actor: "human",
      reason: "looks safe",
    });

    assertSubset(result, {
      decision: "approved",
      approval_id: "apr-001",
      pending_cleared: true,
      grant_recorded: true,
      recommended_next: [{ type: "resume_node", target: "P2-N1" }],
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N1", state: "green_pending", owner_persona: "author" },
      approvals: {
        pending: false,
        pending_count: 0,
        last_grant_scope: "once",
        last_approval_mode: "manual_required",
        active_request: null,
        grants: [{
          approval_id: "apr-001",
          action: "write_file",
          target_ref: "P2-N1",
          risk_class: "critical",
          approval_mode: "manual_required",
          grant_scope: "once",
          decision: "approved",
        }],
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!node || node.status !== "in_progress") {
      throw new Error("expected P2-N1 to resume after approval");
    }

    const sprintStatus = readSprintStatus(rootDir);
    assertSubset(sprintStatus, {
      recommended_next: [{ type: "resume_node", target: "P2-N1" }],
    });

    const journal = readJournalEvents(rootDir);
    const event = journal.find((item) => item.event === "approval_resolved");
    if (!event) {
      throw new Error("expected approval_resolved event");
    }
    assertSubset(event, {
      actor: "human",
      node_id: "P2-N1",
      payload: { approval_id: "apr-001", decision: "approved" },
    });
  },
  "reject-pending-request-keeps-node-blocked": () => {
    const rootDir = makeTempRoot("approval-resolve-reject-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "none",
          last_approval_mode: "never_auto",
          active_request: {
            approval_id: "apr-002",
            action: "edit_file",
            target_ref: "P2-N1",
            risk_class: "high",
            approval_mode: "never_auto",
            grant_scope: "once",
            status: "pending",
            requested_at: "2026-03-09T18:00:00Z",
          },
          grants: [],
        },
      },
      nodes: {
        "P2-N1": {
          status: "blocked",
          approval_ref: "apr-002",
        },
      },
    });

    const result = applyApprovalDecision(rootDir, {
      decision: "rejected",
      approvalId: "apr-002",
      actor: "human",
      reason: "too risky",
    });

    assertSubset(result, {
      decision: "rejected",
      approval_id: "apr-002",
      pending_cleared: true,
      grant_recorded: true,
      recommended_next: [{ type: "human_intervention", target: "P2" }],
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "blocked" },
      node: { active_id: "P2-N1", owner_persona: "human" },
      approvals: {
        pending: false,
        pending_count: 0,
        active_request: null,
        grants: [{ approval_id: "apr-002", decision: "rejected" }],
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!node || node.status !== "blocked") {
      throw new Error("expected P2-N1 to remain blocked after rejection");
    }

    const journal = readJournalEvents(rootDir);
    const event = journal.find((item) => item.event === "approval_resolved");
    if (!event) {
      throw new Error("expected approval_resolved event for rejection");
    }
    assertSubset(event, {
      payload: { approval_id: "apr-002", decision: "rejected" },
    });
  },
  "expire-pending-request-records-expired-and-clears-active-request": () => {
    const rootDir = makeTempRoot("approval-resolve-expire-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "none",
          last_approval_mode: "manual_required",
          active_request: {
            approval_id: "apr-003",
            action: "write_file",
            target_ref: "P2-N1",
            risk_class: "high",
            approval_mode: "manual_required",
            grant_scope: "session",
            status: "pending",
            requested_at: "2026-03-09T18:00:00Z",
          },
          grants: [],
        },
      },
      nodes: {
        "P2-N1": {
          status: "blocked",
          approval_ref: "apr-003",
        },
      },
    });

    const result = applyApprovalDecision(rootDir, {
      decision: "expired",
      approvalId: "apr-003",
      actor: "runtime",
      reason: "expired by SLA",
    });

    assertSubset(result, {
      decision: "expired",
      approval_id: "apr-003",
      pending_cleared: true,
      grant_recorded: false,
      recommended_next: [{ type: "human_intervention", target: "P2" }],
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      approvals: {
        pending: false,
        pending_count: 0,
        active_request: null,
        grants: [],
      },
    });

    const journal = readJournalEvents(rootDir);
    const event = journal.find((item) => item.event === "approval_expired");
    if (!event) {
      throw new Error("expected approval_expired event");
    }
    assertSubset(event, {
      actor: "runtime",
      payload: { approval_id: "apr-003", decision: "expired" },
    });
  },
  "rejects-mismatched-approval-id": () => {
    const rootDir = makeTempRoot("approval-resolve-mismatch-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        approvals: {
          pending: true,
          pending_count: 1,
          active_request: {
            approval_id: "apr-004",
            action: "write_file",
            target_ref: "P2-N1",
            risk_class: "high",
            approval_mode: "manual_required",
            grant_scope: "once",
            status: "pending",
            requested_at: "2026-03-09T18:00:00Z",
          },
        },
      },
      nodes: {
        "P2-N1": { status: "blocked", approval_ref: "apr-004" },
      },
    });

    let failed = false;
    try {
      applyApprovalDecision(rootDir, {
        decision: "approved",
        approvalId: "apr-999",
        actor: "human",
      });
    } catch (error) {
      failed = /approval_id/i.test(String(error.message || ""));
    }
    if (!failed) {
      throw new Error("expected mismatched approval_id to be rejected");
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
    console.log("APPROVAL_RESOLUTION_FIXTURES_PASS");
  }
}

main();
