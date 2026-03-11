"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { createCheckpoint } = require("./checkpoints.cjs");
const { appendEvents, readEvents } = require("./journal.cjs");
const { refreshLedger } = require("./ledger.cjs");
const { getNodeById, reviewAccepted } = require("./runtime-state.cjs");
const {
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("./store.cjs");

function nowIso() {
  return new Date().toISOString();
}

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function decisionPayload(mode, decision) {
  return {
    mode,
    route_verdict: decision.route_verdict,
    active_phase: decision.active_phase,
    active_node: decision.active_node,
    next_persona: decision.next_persona,
    next_capability: decision.next_capability,
    policy_route_effect: decision?.policy_verdict?.route_effect || null,
    policy_primary_reason: decision?.policy_verdict?.primary_reason || null,
    block_reason: decision.block_reason || null,
    recommended_next: clone(decision.recommended_next || []),
  };
}

function firstRecommendedNext(decision) {
  return Array.isArray(decision?.recommended_next) ? decision.recommended_next[0] || null : null;
}

function deriveFailureKind(eventName, decision, actionContext) {
  if (eventName === "node_timed_out") {
    return "timeout";
  }
  return actionContext?.retryContext?.failureKind
    || actionContext?.redEvidence?.failureKind
    || decision?.policy_verdict?.primary_reason
    || decision?.block_reason
    || null;
}

function buildTransitionContext(eventName, decision, actionContext) {
  if (!["node_failed", "node_timed_out"].includes(eventName)) {
    return null;
  }

  const nextAction = firstRecommendedNext(decision);
  const parsedAttempt = Number(nextAction?.params?.next_attempt);
  const retryAttempt = Number.isFinite(parsedAttempt) ? parsedAttempt : null;
  const transitionContext = {
    failure_kind: deriveFailureKind(eventName, decision, actionContext),
  };

  if (
    eventName === "node_timed_out"
    || decision?.policy_verdict?.timeout?.triggered === true
    || actionContext?.timeoutTriggered === true
  ) {
    transitionContext.timeout_triggered = true;
  }
  if (retryAttempt !== null) {
    transitionContext.retry_attempt = retryAttempt;
  }

  return Object.values(transitionContext).some((value) => value !== null && value !== undefined)
    ? transitionContext
    : null;
}

function deriveNodeState(node) {
  const tddState = String(node?.tdd_state || "idle");
  if (!node || node.tdd_required === false) {
    return tddState === "verified" ? "verified" : "idle";
  }
  if (["red_pending", "red_verified", "green_pending", "green_verified", "refactor_pending", "verified"].includes(tddState)) {
    return tddState;
  }
  return "red_pending";
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

function buildNodeSummary(taskGraph) {
  return (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).map((node) => ({
    id: node.id,
    status: node.status,
    tdd_state: node.tdd_state,
  }));
}

function buildSprintStatus(nextSprintStatus, taskGraph, decision) {
  return {
    ...clone(nextSprintStatus || {}),
    schema: 4,
    active_phase: decision.active_phase,
    node_summary: buildNodeSummary(taskGraph),
    recommended_next: clone(decision.recommended_next || []),
  };
}

function shouldFinalizeActiveReviewNode(currentActiveNode, decision) {
  return Boolean(
    currentActiveNode &&
      currentActiveNode.status === "review" &&
      reviewAccepted(currentActiveNode.review_state || {}) &&
      decision?.policy_verdict?.completion?.node_complete_ready === true,
  );
}

function finalizeActiveReviewNode(taskGraph, nodeId) {
  updateNodeStatus(taskGraph, nodeId, (draft) => ({
    ...draft,
    status: "completed",
    tdd_state: draft.tdd_required === false ? "not_applicable" : "verified",
  }));
}

function isTerminalHandoffDecision(decision) {
  const firstAction = Array.isArray(decision?.recommended_next) ? decision.recommended_next[0] : null;
  return Boolean(
    decision?.policy_verdict?.completion?.stop_gate_ready === true &&
      firstAction?.type === "human_intervention" &&
      firstAction?.target === "session",
  );
}

function shouldPersistVerifyTransition(currentActiveNode, decision) {
  if (isTerminalHandoffDecision(decision)) {
    return true;
  }

  if (shouldFinalizeActiveReviewNode(currentActiveNode, decision)) {
    return true;
  }

  const nextAction = firstRecommendedNext(decision);
  return ["enter_phase", "start_node", "request_approval"].includes(nextAction?.type || "");
}

function captureExistingJournal(rootDir) {
  const journalPath = path.join(path.resolve(rootDir), ".ai", "workflow", "journal.jsonl");
  return {
    path: journalPath,
    exists: fs.existsSync(journalPath),
    raw: fs.existsSync(journalPath) ? fs.readFileSync(journalPath, "utf8") : "",
  };
}

function captureExistingLedger(rootDir) {
  const ledgerPath = path.join(path.resolve(rootDir), ".ai", "workflow", "ledger.md");
  return {
    path: ledgerPath,
    exists: fs.existsSync(ledgerPath),
    raw: fs.existsSync(ledgerPath) ? fs.readFileSync(ledgerPath, "utf8") : "",
  };
}

function restoreJournal(snapshot) {
  fs.mkdirSync(path.dirname(snapshot.path), { recursive: true });
  if (snapshot.exists) {
    fs.writeFileSync(snapshot.path, snapshot.raw, "utf8");
    return;
  }
  if (fs.existsSync(snapshot.path)) {
    fs.rmSync(snapshot.path, { force: true });
  }
}

function restoreLedger(snapshot) {
  fs.mkdirSync(path.dirname(snapshot.path), { recursive: true });
  if (snapshot.exists) {
    fs.writeFileSync(snapshot.path, snapshot.raw, "utf8");
    return;
  }
  if (fs.existsSync(snapshot.path)) {
    fs.rmSync(snapshot.path, { force: true });
  }
}

function incrementBudgetCounter(session, key) {
  if (!session?.loop_budget || !(key in session.loop_budget)) {
    return;
  }
  const currentValue = Number(session.loop_budget[key] || 0);
  session.loop_budget[key] = Number.isFinite(currentValue) ? currentValue + 1 : 1;
}

function resolveTargetNode(taskGraph, nextAction, decision, currentActiveNode) {
  const directTarget = nextAction?.target ? getNodeById(taskGraph, nextAction.target) : null;
  if (directTarget) {
    return directTarget;
  }
  const activeTarget = decision?.active_node ? getNodeById(taskGraph, decision.active_node) : null;
  if (activeTarget) {
    return activeTarget;
  }
  return currentActiveNode || null;
}

function buildApprovalId() {
  return `apr-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

function materializeApprovalRequest(session, taskGraph, nextAction, decision, currentActiveNode) {
  const targetNode = resolveTargetNode(taskGraph, nextAction, decision, currentActiveNode);
  const targetRef = nextAction?.target || targetNode?.id || decision?.active_node || session?.phase?.current || "runtime";
  const existing = session?.approvals?.active_request || null;
  const samePendingTarget = session?.approvals?.pending === true && existing && existing.target_ref === targetRef;
  const approvalId = nextAction?.params?.approval_id || existing?.approval_id || targetNode?.approval_ref || buildApprovalId();
  const approvalMode = nextAction?.params?.approval_mode || existing?.approval_mode || "manual_required";
  const grantScope = nextAction?.params?.grant_scope || existing?.grant_scope || "once";
  const riskClass = nextAction?.params?.risk_class || existing?.risk_class || "high";
  const requestedAt = samePendingTarget ? (existing?.requested_at || nowIso()) : nowIso();

  session.approvals.pending = true;
  session.approvals.pending_count = samePendingTarget
    ? Math.max(1, Number(session.approvals.pending_count || 0))
    : Math.max(1, Number(session.approvals.pending_count || 0) + 1);
  session.approvals.last_grant_scope = grantScope;
  session.approvals.last_approval_mode = approvalMode;
  session.approvals.active_request = {
    approval_id: approvalId,
    action: decision?.next_capability || "human_approval",
    target_ref: targetRef,
    risk_class: riskClass,
    approval_mode: approvalMode,
    grant_scope: grantScope,
    status: "pending",
    requested_at: requestedAt,
  };

  if (targetNode) {
    updateNodeStatus(taskGraph, targetNode.id, (draft) => ({
      ...draft,
      approval_ref: approvalId,
    }));
  }

  return {
    approvalId,
    targetNodeId: targetNode?.id || null,
  };
}

function markNodeCompleted(taskGraph, nodeId) {
  updateNodeStatus(taskGraph, nodeId, (draft) => ({
    ...draft,
    status: "completed",
    tdd_state: draft.tdd_required === false ? "not_applicable" : draft.tdd_state === "verified" ? "verified" : "verified",
  }));
}

function markNodeFailed(taskGraph, nodeId) {
  updateNodeStatus(taskGraph, nodeId, (draft) => ({
    ...draft,
    status: "failed",
  }));
}

function computeNextAssets(rootDir, mode, decision, options = {}) {
  const currentSession = readSession(rootDir);
  const currentTaskGraph = readTaskGraph(rootDir);
  const currentSprintStatus = readSprintStatus(rootDir);
  if (!currentSession || !currentTaskGraph) {
    throw new Error("transition-applier requires session.yaml and task-graph.yaml");
  }

  const session = clone(currentSession);
  const taskGraph = clone(currentTaskGraph);
  const currentPhaseId = session.phase?.current || decision.active_phase || null;
  const nextAction = Array.isArray(decision.recommended_next) ? decision.recommended_next[0] : null;
  const currentActiveNode = session?.node?.active_id && session.node.active_id !== "none"
    ? getNodeById(taskGraph, session.node.active_id)
    : null;
  const terminalHandoff = isTerminalHandoffDecision(decision);
  const allowVerifyTransition = mode !== "verify" || options?.allowVerifyTransition === true || terminalHandoff;
  const deferResumeForBackoff = decision?.route_verdict === "hold" && decision?.block_reason === "retry_backoff_pending";
  const emittedEvents = new Set(Array.isArray(decision.emit_events) ? decision.emit_events : []);
  let approvalState = null;

  if (mode === "verify" && (!allowVerifyTransition || !shouldPersistVerifyTransition(currentActiveNode, decision))) {
    return {
      session: currentSession,
      taskGraph: currentTaskGraph,
      sprintStatus: currentSprintStatus,
      filesUpdated: [],
    };
  }

  if (shouldFinalizeActiveReviewNode(currentActiveNode, decision)) {
    finalizeActiveReviewNode(taskGraph, currentActiveNode.id);
  }

  if (decision.active_phase) {
    session.phase.current = decision.active_phase;
  }
  if (decision.next_persona) {
    session.node.owner_persona = decision.next_persona;
  }

  if (emittedEvents.has("node_completed") && currentActiveNode?.id) {
    markNodeCompleted(taskGraph, currentActiveNode.id);
    incrementBudgetCounter(session, "consumed_nodes");
  }

  if (emittedEvents.has("node_failed") && currentActiveNode?.id) {
    markNodeFailed(taskGraph, currentActiveNode.id);
    session.phase.status = "blocked";
    session.node.active_id = currentActiveNode.id;
    session.node.owner_persona = "human";
    session.node.state = "failed";
    incrementBudgetCounter(session, "consumed_failures");
  }

  if (emittedEvents.has("node_timed_out") && currentActiveNode?.id) {
    markNodeFailed(taskGraph, currentActiveNode.id);
    session.phase.status = "blocked";
    session.node.active_id = currentActiveNode.id;
    session.node.owner_persona = "human";
    session.node.state = "failed";
    incrementBudgetCounter(session, "consumed_failures");
  }

  if (emittedEvents.has("approval_requested")) {
    approvalState = materializeApprovalRequest(session, taskGraph, nextAction, decision, currentActiveNode);
  }

  if (terminalHandoff) {
    if (currentPhaseId) {
      updatePhaseStatus(taskGraph, currentPhaseId, "completed");
    }
    session.phase.current = currentPhaseId || decision.active_phase || session.phase.current;
    session.phase.status = "completed";
    session.node.active_id = "none";
    session.node.owner_persona = "human";
    session.node.state = "idle";
  }

  if (nextAction?.type === "enter_phase") {
    if (currentPhaseId && currentPhaseId !== nextAction.target) {
      updatePhaseStatus(taskGraph, currentPhaseId, "completed");
    }
    updatePhaseStatus(taskGraph, nextAction.target, "in_progress");
    session.phase.current = nextAction.target;
    session.phase.status = "in_progress";
    session.node.active_id = "none";
    session.node.owner_persona = decision.next_persona || "planner";
    session.node.state = "idle";
  }

  if (nextAction?.type === "start_node") {
    updatePhaseStatus(taskGraph, session.phase.current, "in_progress");
    updateNodeStatus(taskGraph, nextAction.target, (draft) => ({
      ...draft,
      status: "in_progress",
      owner_persona: decision.next_persona || draft.owner_persona,
    }));
    const node = getNodeById(taskGraph, nextAction.target);
    session.phase.status = "in_progress";
    session.node.active_id = nextAction.target;
    session.node.owner_persona = decision.next_persona || node?.owner_persona || session.node.owner_persona;
    session.node.state = deriveNodeState(node);
  }

  if ((nextAction?.type === "resume_node" || nextAction?.type === "retry_node") && !deferResumeForBackoff) {
    updatePhaseStatus(taskGraph, session.phase.current, "in_progress");
    updateNodeStatus(taskGraph, nextAction.target, (draft) => ({
      ...draft,
      status: draft.status === "review" ? "review" : "in_progress",
      owner_persona: nextAction.params?.persona || decision.next_persona || draft.owner_persona,
    }));
    const node = getNodeById(taskGraph, nextAction.target);
    session.phase.status = node?.status === "review" ? "review" : "in_progress";
    session.node.active_id = nextAction.target;
    session.node.owner_persona = nextAction.params?.persona || decision.next_persona || node?.owner_persona || session.node.owner_persona;
    session.node.state = deriveNodeState(node);
    if (mode === "resume" && decision.block_reason === "restore_pending") {
      session.recovery.restore_pending = false;
      session.recovery.restore_reason = null;
    }
  }

  if (nextAction?.type === "request_approval") {
    approvalState = approvalState || materializeApprovalRequest(session, taskGraph, nextAction, decision, currentActiveNode);
    const targetNode = resolveTargetNode(taskGraph, nextAction, decision, currentActiveNode);
    if (targetNode) {
      updateNodeStatus(taskGraph, targetNode.id, (draft) => ({
        ...draft,
        status: emittedEvents.has("node_failed") || emittedEvents.has("node_timed_out") ? draft.status : "blocked",
        approval_ref: approvalState.approvalId,
      }));
      session.node.active_id = targetNode.id;
      session.node.state = emittedEvents.has("node_failed") || emittedEvents.has("node_timed_out") ? "failed" : deriveNodeState(targetNode);
    }
    updatePhaseStatus(taskGraph, session.phase.current, "blocked");
    session.phase.status = "blocked";
    session.node.owner_persona = decision.next_persona || "human";
  }

  session.timestamps = {
    ...session.timestamps,
    updated_at: nowIso(),
  };

  const sprintStatus = buildSprintStatus(currentSprintStatus, taskGraph, {
    ...decision,
    active_phase: session.phase.current,
  });

  return {
    session,
    taskGraph,
    sprintStatus,
    filesUpdated: [
      ".ai/workflow/session.yaml",
      ".ai/workflow/task-graph.yaml",
      ".ai/workflow/sprint-status.yaml",
      ".ai/workflow/journal.jsonl",
      ".ai/workflow/ledger.md",
    ],
  };
}

function buildJournalInputs(rootDir, mode, decision, sessionOverride, actionContext = {}) {
  const session = sessionOverride || readSession(rootDir);
  const payload = decisionPayload(mode, decision);
  const existingEvents = readEvents(rootDir);
  const eventInputs = [];
  const currentPhase = session?.phase?.current || "none";
  const currentNodeId = session?.node?.active_id || "none";

  if (mode === "run" && existingEvents.length === 0) {
    eventInputs.push({
      runId: session?.run_id,
      event: "session_started",
      phase: payload.active_phase || session?.phase?.current || "none",
      nodeId: payload.active_node || session?.node?.active_id || "none",
      actor: "runtime",
      payload: {
        source: "transition_applier",
        route_decision: payload,
      },
    });
  }

  for (const eventName of decision.emit_events || []) {
    let phase = payload.active_phase || currentPhase;
    let nodeId = payload.active_node || currentNodeId;

    if (["node_completed", "node_failed", "node_timed_out", "node_bypassed", "review_verdict_recorded"].includes(eventName)) {
      nodeId = currentNodeId;
      phase = currentPhase;
    }
    if (eventName === "phase_completed") {
      phase = currentPhase;
      nodeId = currentNodeId;
    }
    if (eventName === "phase_entered") {
      phase = payload.active_phase || currentPhase;
      nodeId = payload.active_node || currentNodeId;
    }
    if (eventName === "node_started") {
      phase = payload.active_phase || currentPhase;
      nodeId = payload.active_node || currentNodeId;
    }

    const eventPayload = {
      source: "transition_applier",
      route_decision: payload,
    };
    const transitionContext = buildTransitionContext(eventName, decision, actionContext);
    if (transitionContext) {
      eventPayload.transition_context = transitionContext;
    }

    eventInputs.push({
      runId: session?.run_id,
      event: eventName,
      phase,
      nodeId,
      actor: "runtime",
      payload: eventPayload,
    });
  }

  return eventInputs;
}

function applyRuntimeTransition(rootDir, options = {}) {
  const mode = options.mode || "run";
  const decision = options.decision || {};
  const currentSession = readSession(rootDir);
  const currentTaskGraph = readTaskGraph(rootDir);
  const currentSprintStatus = readSprintStatus(rootDir);
  const journalSnapshot = captureExistingJournal(rootDir);
  const ledgerSnapshot = captureExistingLedger(rootDir);

  const nextAssets = computeNextAssets(rootDir, mode, decision, options);
  if (mode === "verify" && nextAssets.filesUpdated.length === 0) {
    return nextAssets;
  }

  try {
    writeSession(rootDir, nextAssets.session);
    writeTaskGraph(rootDir, nextAssets.taskGraph);
    writeSprintStatus(rootDir, nextAssets.sprintStatus);
    const events = buildJournalInputs(rootDir, mode, decision, currentSession, options.actionContext || {});
    const createdCheckpoints = [];
    if (events.length > 0) {
      appendEvents(rootDir, events);
    }
    if (events.some((item) => item.event === "node_completed") && currentSession?.node?.active_id && currentSession.node.active_id !== "none") {
      const checkpoint = createCheckpoint(rootDir, {
        checkpointClass: "node",
        phase: currentSession.phase?.current,
        nodeId: currentSession.node.active_id,
        actor: "runtime",
        sourceEvent: "node_completed",
      });
      createdCheckpoints.push({
        checkpoint_id: checkpoint.checkpoint_id,
        checkpoint_class: "node",
      });
    }
    if (events.some((item) => item.event === "review_verdict_recorded") && currentSession?.node?.active_id && currentSession.node.active_id !== "none") {
      const checkpoint = createCheckpoint(rootDir, {
        checkpointClass: "review",
        phase: currentSession.phase?.current,
        nodeId: currentSession.node.active_id,
        actor: "runtime",
        sourceEvent: "review_verdict_recorded",
      });
      createdCheckpoints.push({
        checkpoint_id: checkpoint.checkpoint_id,
        checkpoint_class: "review",
      });
    }
    refreshLedger(rootDir);
    const finalSession = createdCheckpoints.length > 0 ? readSession(rootDir) : nextAssets.session;
    return {
      ...nextAssets,
      session: finalSession,
      filesUpdated: createdCheckpoints.length > 0
        ? [...nextAssets.filesUpdated, ".ai/workflow/checkpoints"]
        : nextAssets.filesUpdated,
      appendedEvents: events.map((item) => item.event),
      createdCheckpoints,
    };
  } catch (error) {
    if (currentSession) {
      writeSession(rootDir, currentSession);
    }
    if (currentTaskGraph) {
      writeTaskGraph(rootDir, currentTaskGraph);
    }
    if (currentSprintStatus) {
      writeSprintStatus(rootDir, currentSprintStatus);
    }
    restoreJournal(journalSnapshot);
    restoreLedger(ledgerSnapshot);
    throw error;
  }
}

module.exports = {
  applyRuntimeTransition,
  buildSprintStatus,
  computeNextAssets,
  decisionPayload,
};
