#!/usr/bin/env node
"use strict";

const path = require("node:path");

const { appendEvent } = require("./journal.cjs");
const { getNodeById } = require("./runtime-state.cjs");
const { runEngineKernel } = require("./engine-kernel.cjs");
const {
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("./store.cjs");

const ALLOWED_DECISIONS = new Set(["approved", "rejected", "expired"]);
const ALLOWED_ACTORS = new Set(["human", "runtime"]);

function nowIso() {
  return new Date().toISOString();
}

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function updateNodeStatus(taskGraph, nodeId, updater) {
  if (!Array.isArray(taskGraph?.nodes)) {
    return;
  }
  taskGraph.nodes = taskGraph.nodes.map((node) => (node.id === nodeId ? updater({ ...node }) : node));
}

function updatePhaseStatus(taskGraph, phaseId, status) {
  if (!Array.isArray(taskGraph?.phases)) {
    return;
  }
  taskGraph.phases = taskGraph.phases.map((phase) => (phase.id === phaseId ? { ...phase, status } : phase));
}

function findNodeByApprovalRef(taskGraph, approvalId) {
  return (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).find((node) => node?.approval_ref === approvalId) || null;
}

function buildGrantRecord(activeRequest, decision, decisionAt, expiresAt) {
  return {
    approval_id: activeRequest.approval_id,
    grant_scope: activeRequest.grant_scope,
    approval_mode: activeRequest.approval_mode,
    action: activeRequest.action,
    target_ref: activeRequest.target_ref,
    risk_class: activeRequest.risk_class,
    decision,
    granted_at: decisionAt,
    expires_at: expiresAt || null,
  };
}

function upsertGrant(grants, record) {
  const next = Array.isArray(grants) ? grants.filter((item) => item?.approval_id !== record.approval_id) : [];
  next.push(record);
  return next;
}

function resolvePendingApprovalState(rootDir, options = {}) {
  const decision = String(options.decision || "").trim().toLowerCase();
  if (!ALLOWED_DECISIONS.has(decision)) {
    throw new Error(`invalid approval decision: ${options.decision}`);
  }
  const actor = String(options.actor || "human").trim().toLowerCase();
  if (!ALLOWED_ACTORS.has(actor)) {
    throw new Error(`invalid actor: ${options.actor}`);
  }

  const currentSession = readSession(rootDir);
  const currentTaskGraph = readTaskGraph(rootDir);
  const currentSprintStatus = readSprintStatus(rootDir);
  if (!currentSession || !currentTaskGraph || !currentSprintStatus) {
    throw new Error("approval-resolution requires session.yaml, task-graph.yaml, and sprint-status.yaml");
  }

  const activeRequest = currentSession?.approvals?.active_request;
  if (!currentSession?.approvals?.pending || !activeRequest) {
    throw new Error("no pending approval request to resolve");
  }
  if (activeRequest.status !== "pending") {
    throw new Error(`active approval is not pending: ${activeRequest.status}`);
  }
  if (options.approvalId && String(options.approvalId) !== String(activeRequest.approval_id)) {
    throw new Error(`approval_id mismatch: expected ${activeRequest.approval_id} but got ${options.approvalId}`);
  }

  const session = clone(currentSession);
  const taskGraph = clone(currentTaskGraph);
  const decisionAt = nowIso();
  const expiresAt = options.expiresAt || null;
  const targetNode = getNodeById(taskGraph, activeRequest.target_ref) || findNodeByApprovalRef(taskGraph, activeRequest.approval_id);

  session.approvals.pending = false;
  session.approvals.pending_count = 0;
  session.approvals.last_grant_scope = activeRequest.grant_scope || session.approvals.last_grant_scope || "none";
  session.approvals.last_approval_mode = activeRequest.approval_mode || session.approvals.last_approval_mode || "none";
  session.approvals.active_request = null;

  if (decision === "approved") {
    session.approvals.grants = upsertGrant(session.approvals.grants, buildGrantRecord(activeRequest, decision, decisionAt, expiresAt));
    if (targetNode) {
      updateNodeStatus(taskGraph, targetNode.id, (draft) => ({
        ...draft,
        status: "in_progress",
        approval_ref: activeRequest.approval_id,
      }));
      updatePhaseStatus(taskGraph, targetNode.phase_id, "in_progress");
      session.phase.current = targetNode.phase_id;
      session.phase.status = "in_progress";
      session.node.active_id = targetNode.id;
      session.node.owner_persona = targetNode.owner_persona || currentSession?.node?.owner_persona || "author";
      session.node.state = targetNode.tdd_required === false
        ? (targetNode.tdd_state === "verified" ? "verified" : "idle")
        : (targetNode.tdd_state || currentSession?.node?.state || "red_pending");
    }
  } else {
    if (decision === "rejected") {
      session.approvals.grants = upsertGrant(session.approvals.grants, buildGrantRecord(activeRequest, decision, decisionAt, expiresAt));
    }
    if (targetNode) {
      updateNodeStatus(taskGraph, targetNode.id, (draft) => ({
        ...draft,
        status: "blocked",
        approval_ref: activeRequest.approval_id,
      }));
      updatePhaseStatus(taskGraph, targetNode.phase_id, "blocked");
      session.phase.current = targetNode.phase_id;
      session.phase.status = "blocked";
      session.node.active_id = targetNode.id;
      session.node.owner_persona = "human";
    }
  }

  session.timestamps = {
    ...session.timestamps,
    updated_at: decisionAt,
  };

  writeSession(rootDir, session);
  writeTaskGraph(rootDir, taskGraph);
  if (currentSprintStatus) {
    writeSprintStatus(rootDir, currentSprintStatus);
  }

  const eventName = decision === "expired" ? "approval_expired" : "approval_resolved";
  appendEvent(rootDir, {
    event: eventName,
    actor,
    phase: session.phase?.current || currentSession.phase?.current || "none",
    nodeId: targetNode?.id || currentSession.node?.active_id || "none",
    payload: {
      approval_id: activeRequest.approval_id,
      decision,
      action: activeRequest.action,
      target_ref: activeRequest.target_ref,
      risk_class: activeRequest.risk_class,
      approval_mode: activeRequest.approval_mode,
      grant_scope: activeRequest.grant_scope,
      reason: options.reason || null,
      resolved_at: decisionAt,
      expires_at: expiresAt,
    },
  });

  let routeVerdict = "hold";
  let recommendedNext = [];
  if (decision === "approved") {
    const refreshed = runEngineKernel(rootDir, {
      syncSprintStatus: true,
      actionContext: {},
      specRootDir: rootDir,
    });
    const refreshedSprintStatus = readSprintStatus(rootDir);
    routeVerdict = refreshed.route_verdict;
    recommendedNext = clone(refreshedSprintStatus?.recommended_next || []);
  } else {
    recommendedNext = [{
      type: "human_intervention",
      target: session.phase?.current || currentSession.phase?.current || "session",
      params: {},
      reason: decision === "expired" ? "approval expired; human decision required" : "approval rejected; human decision required",
      blocking_on: [],
      priority: "now",
    }];
    writeSprintStatus(rootDir, {
      ...clone(currentSprintStatus || {}),
      schema: 4,
      active_phase: session.phase?.current || currentSession.phase?.current || null,
      node_summary: (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).map((node) => ({
        id: node.id,
        status: node.status,
        tdd_state: node.tdd_state,
      })),
      recommended_next: recommendedNext,
    });
  }

  return {
    decision,
    approval_id: activeRequest.approval_id,
    target_ref: activeRequest.target_ref,
    target_node_id: targetNode?.id || null,
    event: eventName,
    actor,
    pending_cleared: session.approvals.pending === false && session.approvals.pending_count === 0,
    grant_recorded: decision !== "expired",
    recommended_next: recommendedNext,
    route_verdict: routeVerdict,
    files_updated: [
      ".ai/workflow/session.yaml",
      ".ai/workflow/task-graph.yaml",
      ".ai/workflow/sprint-status.yaml",
      ".ai/workflow/journal.jsonl",
    ],
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    decision: null,
    approvalId: null,
    actor: "human",
    reason: null,
    expiresAt: null,
    json: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--decision":
        index += 1;
        parsed.decision = argv[index];
        break;
      case "--approval-id":
        index += 1;
        parsed.approvalId = argv[index];
        break;
      case "--actor":
        index += 1;
        parsed.actor = argv[index];
        break;
      case "--reason":
        index += 1;
        parsed.reason = argv[index];
        break;
      case "--expires-at":
        index += 1;
        parsed.expiresAt = argv[index];
        break;
      case "--json":
        parsed.json = true;
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }
  return parsed;
}

function formatHumanSummary(result) {
  const next = Array.isArray(result.recommended_next) && result.recommended_next[0]
    ? `${result.recommended_next[0].type}:${result.recommended_next[0].target}`
    : "none";
  return [
    `[approval-resolution] 已写回审批结果：${result.decision}`,
    `approval_id=${result.approval_id}`,
    `target=${result.target_ref}`,
    `next=${next}`,
  ].join(" ");
}

if (require.main === module) {
  try {
    const parsed = parseArgs(process.argv.slice(2));
    const result = resolvePendingApprovalState(parsed.rootDir, parsed);
    if (parsed.json) {
      console.log(JSON.stringify(result, null, 2));
    } else {
      console.log(formatHumanSummary(result));
    }
  } catch (error) {
    console.error(`[approval-resolution] ${error.message}`);
    process.exit(1);
  }
}

module.exports = {
  resolvePendingApprovalState,
  applyApprovalDecision: resolvePendingApprovalState,
};
