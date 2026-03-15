"use strict";

const {
  areDependenciesCompleted,
  evaluateStateExpression,
  getActiveNode,
  getActivePhase,
  listPhaseNodes,
  phaseIsComplete,
  priorityRank,
  reviewAccepted,
  reviewFailed,
} = require("./runtime-state.cjs");

function buildRouteBasis() {
  return {
    session_fields: [],
    phase_checks: [],
    node_checks: [],
    policy_verdicts: [],
    blockers: [],
    sorting_decision: [],
  };
}

function buildResult(base) {
  return {
    route_verdict: base.route_verdict,
    active_phase: base.active_phase,
    active_node: base.active_node,
    next_persona: base.next_persona,
    next_capability: base.next_capability,
    recommended_next: base.recommended_next || [],
    block_reason: base.block_reason || null,
    route_basis: base.route_basis,
    emit_events: base.emit_events || [],
    bypassed_nodes: base.bypassed_nodes || [],
  };
}

function validatorBlocks(validatorVerdict) {
  if (!validatorVerdict || typeof validatorVerdict !== "object") {
    return false;
  }
  return ["REWORK", "FAIL"].includes(String(validatorVerdict.verdict));
}

function requestApprovalNext(target, reason, policyVerdict) {
  return {
    type: "request_approval",
    target,
    params: {
      risk_class: policyVerdict?.approval?.risk_class || null,
      approval_mode: policyVerdict?.approval?.approval_mode || null,
      grant_scope: policyVerdict?.approval?.allowed_grant_scopes?.[0] || null,
    },
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function humanNext(target, reason) {
  return {
    type: "human_intervention",
    target,
    params: {},
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function resumeNodeNext(target, params, reason) {
  return {
    type: "resume_node",
    target,
    params: params || {},
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function startNodeNext(target, reason) {
  return {
    type: "start_node",
    target,
    params: {},
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function enterPhaseNext(target, reason) {
  return {
    type: "enter_phase",
    target,
    params: {},
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function retryNodeNext(target, policyVerdict, reason) {
  return {
    type: "retry_node",
    target,
    params: {
      next_attempt: policyVerdict?.retry?.next_attempt || null,
      wait_until: policyVerdict?.retry?.retry_due_at || null,
    },
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function restoreRequiresHuman(reason) {
  const normalized = String(reason || "").trim();
  return [
    "missing_tool_call_metadata",
    "target_snapshot_metadata_missing",
    "target_snapshot_content_missing",
    "target_snapshot_unsupported_kind",
    "target_snapshot_requires_manual_restore",
  ].includes(normalized);
}

function terminalHandoffNext(reason) {
  return {
    type: "human_intervention",
    target: "session",
    params: { handoff: "terminal" },
    reason,
    blocking_on: [],
    priority: "now",
  };
}

function nextPhase(taskGraph, currentPhaseId) {
  const phases = Array.isArray(taskGraph?.phases) ? taskGraph.phases : [];
  const currentIndex = phases.findIndex((phase) => phase.id === currentPhaseId);
  return currentIndex >= 0 ? phases[currentIndex + 1] || null : null;
}

function phaseDependencySatisfied(taskGraph, dependencyId, currentPhaseId, currentPhaseCompletable) {
  if (dependencyId === currentPhaseId) {
    return currentPhaseCompletable;
  }
  return areDependenciesCompleted(taskGraph, [dependencyId]);
}

function computeRouteScore(node, taskGraph) {
  // F3: Numeric route score — weighted composite for secondary sort tiebreaker
  // Weights: priority_rank 0.6, dependency_depth 0.2, recency_bonus 0.2
  const rank = priorityRank(node.priority); // 0=critical,1=high,2=medium,3=low
  const maxRank = 3;
  const priorityWeight = ((maxRank - rank) / maxRank) * 0.6;
  const depDepth = Array.isArray(node.depends_on) ? node.depends_on.length : 0;
  const depWeight = (1 / (1 + depDepth)) * 0.2;
  const recencyBonus = 0.2; // static placeholder; future: derive from journal
  return priorityWeight + depWeight + recencyBonus;
}

function selectConcurrentNodes(candidateNodes, maxConcurrent, taskGraph) {
  // F1: Select up to maxConcurrent nodes with no shared file targets and no mutual dependencies
  const selected = [];
  const usedTargets = new Set();
  const selectedIds = new Set();

  for (const node of candidateNodes) {
    if (selected.length >= maxConcurrent) break;
    const target = node.target || null;
    // Check shared file target conflict
    if (target && usedTargets.has(target)) continue;
    // Check mutual dependency conflict
    const hasMutualDep = selected.some((s) =>
      (node.depends_on || []).includes(s.id) || (s.depends_on || []).includes(node.id)
    );
    if (hasMutualDep) continue;
    if (target) usedTargets.add(target);
    selectedIds.add(node.id);
    selected.push(node);
  }
  return selected;
}

function collectReadyNodes(taskGraph, activePhaseId, context, routeBasis) {
  const candidates = listPhaseNodes(taskGraph, activePhaseId)
    .filter((node) => node.status === "ready")
    .filter((node) => areDependenciesCompleted(taskGraph, node.depends_on));
  const readyNodes = [];
  const bypassedNodes = [];

  for (const node of candidates) {
    if (!node.condition) {
      readyNodes.push(node);
      continue;
    }
    const conditionPassed = evaluateStateExpression(node.condition.expression, context);
    if (routeBasis) {
      routeBasis.node_checks.push(`condition:${node.id}=${conditionPassed ? "true" : "false"}`);
    }
    if (!conditionPassed) {
      bypassedNodes.push(node.id);
      continue;
    }
    readyNodes.push(node);
  }

  readyNodes.sort((left, right) => {
    // Primary: priority rank
    const rankDiff = priorityRank(left.priority) - priorityRank(right.priority);
    if (rankDiff !== 0) return rankDiff;
    // Secondary: numeric route score (F3) — higher score first
    const scoreDiff = computeRouteScore(right, taskGraph) - computeRouteScore(left, taskGraph);
    if (scoreDiff !== 0) return scoreDiff;
    // Tertiary: stable node id tie-breaker
    return left.id.localeCompare(right.id);
  });

  return { readyNodes, bypassedNodes };
}

function evaluateRouter(input) {
  const session = input.session || {};
  const taskGraph = input.taskGraph || {};
  const sprintStatus = input.sprintStatus || {};
  const validatorVerdict = input.validatorVerdict || { verdict: "PASS", issues: [] };
  const policyVerdict = input.policyVerdict || { route_effect: "allow", approval: { required: false } };
  const routeBasis = buildRouteBasis();
  const activePhase = getActivePhase(session, taskGraph);
  const activeNode = getActiveNode(session, taskGraph);
  const phaseId = activePhase?.id || session?.phase?.current || sprintStatus?.active_phase || null;
  const context = { session, policy: policyVerdict };
  const activeNodePersona = session?.node?.owner_persona || activeNode?.owner_persona || "author";

  routeBasis.session_fields.push(`phase.current=${phaseId}`);
  routeBasis.session_fields.push(`node.active_id=${session?.node?.active_id || "none"}`);

  if (validatorBlocks(validatorVerdict)) {
    routeBasis.blockers.push("invalid_state");
    return buildResult({
      route_verdict: "block",
      active_phase: phaseId,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [humanNext(phaseId || "runtime", "repair invalid runtime state")],
      block_reason: "invalid_state",
      route_basis: routeBasis,
    });
  }

  if (session?.approvals?.pending === true) {
    routeBasis.blockers.push("approval_pending");
    const target = session?.approvals?.active_request?.target_ref || activeNode?.id || phaseId;
    return buildResult({
      route_verdict: "block",
      active_phase: phaseId,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [requestApprovalNext(target, "resolve pending approval", policyVerdict)],
      block_reason: "approval_pending",
      route_basis: routeBasis,
    });
  }

  if (session?.recovery?.restore_pending === true) {
    routeBasis.blockers.push("restore_pending");
    const restoreReason = String(session?.recovery?.restore_reason || "restore_pending").trim();
    if (restoreRequiresHuman(restoreReason)) {
      return buildResult({
        route_verdict: "block",
        active_phase: phaseId,
        active_node: activeNode?.id || session?.node?.active_id || "none",
        next_persona: "human",
        next_capability: "human_approval",
        recommended_next: [humanNext(activeNode?.id || session?.node?.active_id || phaseId || "runtime", "resolve restore blocker before resume")],
        block_reason: "restore_requires_human",
        route_basis: routeBasis,
      });
    }
    return buildResult({
      route_verdict: "resume",
      active_phase: phaseId,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: activeNode?.owner_persona || "author",
      next_capability: activeNode?.capability || "code_edit",
      recommended_next: [resumeNodeNext(activeNode?.id || session?.node?.active_id || "none", {}, "restore interrupted node")],
      block_reason: "restore_pending",
      route_basis: routeBasis,
      emit_events: ["session_resumed"],
    });
  }

  if (policyVerdict?.route_effect === "require_human") {
    routeBasis.policy_verdicts.push(`policy.route_effect=${policyVerdict.route_effect}`);
    const target = policyVerdict.target_ref || activeNode?.target || activeNode?.id || phaseId;
    const emitEvents = policyVerdict.primary_reason === "node_timed_out"
      ? ["node_timed_out", "approval_requested"]
      : ["approval_requested"];
    return buildResult({
      route_verdict: "block",
      active_phase: phaseId,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [requestApprovalNext(target, policyVerdict.primary_reason || "human approval required", policyVerdict)],
      block_reason: policyVerdict.primary_reason || "approval_required",
      route_basis: routeBasis,
      emit_events: emitEvents,
    });
  }

  if (Number(session?.loop_budget?.consumed_nodes || 0) >= Number(session?.loop_budget?.max_nodes || 0) || Number(session?.loop_budget?.consumed_failures || 0) >= Number(session?.loop_budget?.max_failures || 0)) {
    routeBasis.blockers.push("budget_exhausted");
    return buildResult({
      route_verdict: "block",
      active_phase: phaseId,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [humanNext(phaseId || "runtime", "budget exhausted")],
      block_reason: "budget_exhausted",
      route_basis: routeBasis,
      emit_events: ["budget_exhausted"],
    });
  }

  if (activePhase && !areDependenciesCompleted(taskGraph, activePhase.depends_on || [])) {
    routeBasis.blockers.push("phase_dependency_unmet");
    return buildResult({
      route_verdict: "block",
      active_phase: activePhase.id,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [humanNext(activePhase.id, "resolve phase dependency before advance")],
      block_reason: "phase_dependency_unmet",
      route_basis: routeBasis,
    });
  }

  if (activeNode && reviewFailed(activeNode.review_state)) {
    routeBasis.blockers.push("review_failed");
    return buildResult({
      route_verdict: "block",
      active_phase: phaseId,
      active_node: activeNode.id,
      next_persona: "author",
      next_capability: "code_edit",
      recommended_next: [resumeNodeNext(activeNode.id, { persona: "author" }, "address review findings")],
      block_reason: "review_failed",
      route_basis: routeBasis,
    });
  }

  if (activeNode && activeNode.status === "failed") {
    if (policyVerdict?.retry?.allowed === true) {
      return buildResult({
        route_verdict: "resume",
        active_phase: phaseId,
        active_node: activeNode.id,
        next_persona: "author",
        next_capability: activeNode.capability || "code_edit",
        recommended_next: [retryNodeNext(activeNode.id, policyVerdict, "retry failed node")],
        block_reason: null,
        route_basis: routeBasis,
        emit_events: ["node_started"],
      });
    }
    if (policyVerdict?.retry?.reasons?.includes("retry_backoff_pending")) {
      return buildResult({
        route_verdict: "hold",
        active_phase: phaseId,
        active_node: activeNode.id,
        next_persona: "author",
        next_capability: activeNode.capability || "code_edit",
        recommended_next: [resumeNodeNext(activeNode.id, {
          persona: "author",
          wait_until: policyVerdict?.retry?.retry_due_at || null,
        }, "wait until retry backoff window opens")],
        block_reason: "retry_backoff_pending",
        route_basis: routeBasis,
      });
    }
    routeBasis.blockers.push("failed_node_without_recovery_path");
    return buildResult({
      route_verdict: "block",
      active_phase: phaseId,
      active_node: activeNode.id,
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [humanNext(activeNode.id, "failed node requires manual recovery path")],
      block_reason: "failed_node_without_recovery_path",
      route_basis: routeBasis,
    });
  }

  if (activeNode && activeNode.status === "in_progress") {
    return buildResult({
      route_verdict: "resume",
      active_phase: phaseId,
      active_node: activeNode.id,
      next_persona: activeNodePersona,
      next_capability: activeNode.capability || "code_edit",
      recommended_next: [resumeNodeNext(activeNode.id, { persona: activeNodePersona }, "continue active node")],
      route_basis: routeBasis,
    });
  }

  if (activeNode && activeNode.status === "review") {
    if (activeNode.review_state?.spec_review === "pending") {
      return buildResult({
        route_verdict: "handoff",
        active_phase: phaseId,
        active_node: activeNode.id,
        next_persona: "spec_reviewer",
        next_capability: "spec_review",
        recommended_next: [resumeNodeNext(activeNode.id, { persona: "spec_reviewer" }, "spec review required")],
        route_basis: routeBasis,
      });
    }
    if ((activeNode.review_state?.spec_review === "pass" || activeNode.review_state?.spec_review === "concerns") && activeNode.review_state?.quality_review === "pending") {
      return buildResult({
        route_verdict: "handoff",
        active_phase: phaseId,
        active_node: activeNode.id,
        next_persona: "quality_reviewer",
        next_capability: "quality_review",
        recommended_next: [resumeNodeNext(activeNode.id, { persona: "quality_reviewer" }, "quality review required")],
        route_basis: routeBasis,
      });
    }
    if (reviewAccepted(activeNode.review_state || {}) && policyVerdict?.completion?.node_complete_ready !== true) {
      routeBasis.blockers.push("node_completion_incomplete");
      return buildResult({
        route_verdict: "hold",
        active_phase: phaseId,
        active_node: activeNode.id,
        next_persona: activeNodePersona,
        next_capability: activeNode.capability || "quality_review",
        recommended_next: [resumeNodeNext(activeNode.id, { persona: activeNodePersona }, "complete node verification before advance")],
        block_reason: "node_completion_incomplete",
        route_basis: routeBasis,
      });
    }
  }

  const activeReviewNodeCompleted =
    activeNode &&
    activeNode.status === "review" &&
    reviewAccepted(activeNode.review_state || {}) &&
    policyVerdict?.completion?.node_complete_ready === true;

  if (activePhase && (policyVerdict?.completion?.phase_complete_ready === true || phaseIsComplete(taskGraph, activePhase.id))) {
    const next = nextPhase(taskGraph, activePhase.id);
    const currentPhaseCompletable = policyVerdict?.completion?.phase_complete_ready === true || phaseIsComplete(taskGraph, activePhase.id);
    const nextReady = next && next.status !== "completed" && (next.depends_on || []).every((dependencyId) => phaseDependencySatisfied(taskGraph, dependencyId, activePhase.id, currentPhaseCompletable));
    if (nextReady) {
      return buildResult({
        route_verdict: "advance",
        active_phase: next.id,
        active_node: "none",
        next_persona: "planner",
        next_capability: "planning",
        recommended_next: [enterPhaseNext(next.id, "enter next ready phase")],
        route_basis: routeBasis,
        emit_events: activeReviewNodeCompleted ? ["node_completed", "phase_completed", "phase_entered"] : ["phase_completed", "phase_entered"],
      });
    }
  }

  if (policyVerdict?.completion?.stop_gate_ready === true) {
    const emitEvents = [];
    if (activeReviewNodeCompleted) {
      emitEvents.push("node_completed");
    }
    if (activePhase?.status !== "completed") {
      emitEvents.push("phase_completed");
    }
    emitEvents.push("session_stopped");
    return buildResult({
      route_verdict: "hold",
      active_phase: phaseId,
      active_node: activeNode?.id || session?.node?.active_id || "none",
      next_persona: "human",
      next_capability: "human_approval",
      recommended_next: [terminalHandoffNext("session complete; handoff to human")],
      route_basis: routeBasis,
      emit_events: emitEvents,
    });
  }

  const { readyNodes: candidateNodes, bypassedNodes } = collectReadyNodes(taskGraph, phaseId, context, routeBasis);
  if (candidateNodes.length > 0) {
    // F1: Multi-node scheduling — max_concurrent_nodes defaults to 1 (backward-compatible)
    const maxConcurrent = Number(session?.loop_budget?.max_concurrent_nodes || 1);
    const emitEvents = [];
    if (activeReviewNodeCompleted) {
      emitEvents.push("node_completed");
    }
    if (bypassedNodes.length > 0) {
      emitEvents.push("node_bypassed");
    }
    emitEvents.push("node_started");

    if (maxConcurrent <= 1) {
      // Single-node path (default)
      const candidate = candidateNodes[0];
      routeBasis.sorting_decision.push(`selected=${candidate.id}`);
      return buildResult({
        route_verdict: "advance",
        active_phase: phaseId,
        active_node: candidate.id,
        next_persona: "author",
        next_capability: candidate.capability || "code_edit",
        recommended_next: [startNodeNext(candidate.id, "start highest priority ready node")],
        route_basis: routeBasis,
        emit_events: emitEvents,
        bypassed_nodes: bypassedNodes,
      });
    }

    // Multi-node path: select up to maxConcurrent nodes with no shared targets and no mutual deps
    const selected = selectConcurrentNodes(candidateNodes, maxConcurrent, taskGraph);
    if (selected.length === 0) {
      // Conflict detected — block via invalid_state per spec
      routeBasis.blockers.push("invalid_state");
      return buildResult({
        route_verdict: "block",
        active_phase: phaseId,
        active_node: activeNode?.id || session?.node?.active_id || "none",
        next_persona: "human",
        next_capability: "human_approval",
        recommended_next: [humanNext(phaseId || "runtime", "parallel node conflict: shared file targets")],
        block_reason: "invalid_state",
        route_basis: routeBasis,
      });
    }
    selected.forEach((n) => routeBasis.sorting_decision.push(`selected=${n.id}`));
    return buildResult({
      route_verdict: "advance",
      active_phase: phaseId,
      active_node: selected[0].id,
      next_persona: "author",
      next_capability: selected[0].capability || "code_edit",
      recommended_next: selected.map((n) => startNodeNext(n.id, "start concurrent ready node")),
      route_basis: routeBasis,
      emit_events: emitEvents,
      bypassed_nodes: bypassedNodes,
    });
  }

  return buildResult({
    route_verdict: "hold",
    active_phase: phaseId,
    active_node: activeNode?.id || session?.node?.active_id || "none",
    next_persona: activeNode?.owner_persona || "human",
    next_capability: activeNode?.capability || "human_approval",
    recommended_next: [humanNext(phaseId || "runtime", "no executable next node")],
    route_basis: routeBasis,
    emit_events: bypassedNodes.length > 0 ? ["node_bypassed"] : [],
    bypassed_nodes: bypassedNodes,
  });
}

module.exports = {
  evaluateRouter,
};
