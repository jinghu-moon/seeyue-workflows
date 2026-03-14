"use strict";

const { readJournalEvents, readSession, readSprintStatus, readTaskGraph, writeSprintStatus } = require("./store.cjs");
const { evaluatePolicy } = require("./policy.cjs");
const { evaluateRouter } = require("./router.cjs");
const { getActivePhase, getActiveNode } = require("./runtime-state.cjs");
const { deriveRuntimeSignals } = require("./scheduler.cjs");
const { loadWorkflowSpecs } = require("./workflow-specs.cjs");

const ISSUE_SEVERITY = {
  session_missing: "fail",
  task_graph_missing: "fail",
  sprint_status_missing: "fail",
  active_phase_missing: "fail",
  active_node_missing: "fail",
  approval_queue_exceeded: "rework",
  multi_active_phase_not_supported: "rework",
  multi_active_node_not_supported: "rework",
  parallel_group_not_supported_v1: "rework",
};

function issueSeverity(issue) {
  return ISSUE_SEVERITY[issue] || "fail";
}

function buildValidatorVerdict(issues) {
  if (!Array.isArray(issues) || issues.length === 0) {
    return {
      verdict: "PASS",
      issues: [],
      issue_summary: [],
    };
  }
  const issueSummary = issues.map((issue) => ({ code: issue, severity: issueSeverity(issue) }));
  const severities = issueSummary.map((entry) => entry.severity);
  if (severities.includes("fail")) {
    return {
      verdict: "FAIL",
      issues,
      issue_summary: issueSummary,
    };
  }
  if (severities.includes("rework")) {
    return {
      verdict: "REWORK",
      issues,
      issue_summary: issueSummary,
    };
  }
  if (severities.includes("concerns")) {
    return {
      verdict: "CONCERNS",
      issues,
      issue_summary: issueSummary,
    };
  }
  return {
    verdict: "FAIL",
    issues,
    issue_summary: issueSummary,
  };
}

function validateRuntimeState(session, taskGraph, sprintStatus) {
  const issues = [];
  if (!session || typeof session !== "object") {
    issues.push("session_missing");
  }
  if (!taskGraph || typeof taskGraph !== "object") {
    issues.push("task_graph_missing");
  }
  if (!sprintStatus || typeof sprintStatus !== "object") {
    issues.push("sprint_status_missing");
  }
  if (session && taskGraph) {
    if (!getActivePhase(session, taskGraph)) {
      issues.push("active_phase_missing");
    }
    if (session?.node?.active_id && session.node.active_id !== "none" && !getActiveNode(session, taskGraph)) {
      issues.push("active_node_missing");
    }
    if (Number(session?.approvals?.pending_count || 0) > Number(session?.loop_budget?.max_pending_approvals || 0)) {
      issues.push("approval_queue_exceeded");
    }
    const activePhases = (Array.isArray(taskGraph?.phases) ? taskGraph.phases : []).filter((phase) =>
      ["in_progress", "blocked", "review"].includes(phase?.status),
    );
    if (activePhases.length > 1) {
      issues.push("multi_active_phase_not_supported");
    }
    const activeNodes = (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).filter((node) =>
      ["in_progress", "review"].includes(node?.status),
    );
    if (activeNodes.length > 1) {
      issues.push("multi_active_node_not_supported");
    }
    const reservedParallelGroups = (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).filter((node) =>
      typeof node?.parallel_group === "string" && node.parallel_group.trim().length > 0,
    );
    if (reservedParallelGroups.length > 0) {
      issues.push("parallel_group_not_supported_v1");
    }
  }
  return buildValidatorVerdict(issues);
}

function buildNodeSummary(taskGraph) {
  return (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).map((node) => ({
    id: node.id,
    status: node.status,
    tdd_state: node.tdd_state,
  }));
}

function synchronizeSprintStatus(rootDir, currentSprintStatus, taskGraph, routerVerdict) {
  const nextSprintStatus = {
    ...structuredClone(currentSprintStatus || {}),
    schema: 4,
    active_phase: routerVerdict.active_phase,
    node_summary: buildNodeSummary(taskGraph),
    recommended_next: routerVerdict.recommended_next,
  };
  writeSprintStatus(rootDir, nextSprintStatus);
  return nextSprintStatus;
}

function deriveRetryContextFromJournal(session, taskGraph, actionContext, journalEvents, schedulerSignals) {
  const retryWindow = schedulerSignals?.retry || null;
  if (actionContext?.retryContext) {
    if (!retryWindow?.tracked) {
      return actionContext;
    }
    return {
      ...(actionContext || {}),
      retryContext: {
        ...actionContext.retryContext,
        backoffSatisfied: retryWindow.ready,
        retryDueAt: retryWindow.retry_due_at || null,
        retryWaitMs: retryWindow.remaining_ms ?? null,
      },
    };
  }

  const activeNode = getActiveNode(session, taskGraph);
  if (!activeNode || activeNode.status !== "failed" || !activeNode.retry_policy) {
    return actionContext;
  }

  const events = Array.isArray(journalEvents) ? journalEvents : [];
  const failureEvents = events.filter((event) =>
    event &&
    event.run_id === session?.run_id &&
    event.node_id === activeNode.id &&
    ["node_failed", "node_timed_out"].includes(event.event),
  );

  if (failureEvents.length === 0) {
    return actionContext;
  }

  const latestFailure = failureEvents[failureEvents.length - 1];
  const transitionContext = latestFailure?.payload?.transition_context || {};
  const failureKind = latestFailure.event === "node_timed_out"
    ? "timeout"
    : transitionContext.failure_kind
      || latestFailure?.payload?.route_decision?.policy_primary_reason
      || latestFailure?.payload?.route_decision?.block_reason
      || null;

  if (!failureKind) {
    return actionContext;
  }

  return {
    ...(actionContext || {}),
    retryContext: {
      attemptsUsed: failureEvents.length,
      failureKind,
      source: "journal",
      backoffSatisfied: retryWindow?.tracked ? retryWindow.ready : true,
      retryDueAt: retryWindow?.retry_due_at || null,
      retryWaitMs: retryWindow?.remaining_ms ?? null,
    },
  };
}

function mergeTimeoutContext(actionContext, schedulerSignals) {
  if (actionContext?.timeoutTriggered === true) {
    return actionContext;
  }
  if (schedulerSignals?.timeout?.triggered !== true) {
    return actionContext;
  }
  return {
    ...(actionContext || {}),
    timeoutTriggered: true,
    timeoutContext: {
      startedAt: schedulerSignals.timeout.started_at,
      deadlineAt: schedulerSignals.timeout.deadline_at,
      timeoutSeconds: schedulerSignals.timeout.timeout_seconds,
      graceSeconds: schedulerSignals.timeout.grace_seconds,
    },
  };
}

function runEngineKernel(rootDir, options = {}) {
  const session = readSession(rootDir);
  const taskGraph = readTaskGraph(rootDir);
  const sprintStatus = readSprintStatus(rootDir);
  const journalEvents = readJournalEvents(rootDir);
  const specs = loadWorkflowSpecs(options.specRootDir || rootDir);
  const validatorVerdict = validateRuntimeState(session, taskGraph, sprintStatus);
  const schedulerSignals = deriveRuntimeSignals(session, taskGraph, journalEvents, { now: options.now });
  const actionContext = mergeTimeoutContext(
    deriveRetryContextFromJournal(session, taskGraph, options.actionContext || {}, journalEvents, schedulerSignals),
    schedulerSignals,
  );
  const policyVerdict = evaluatePolicy({
    session,
    taskGraph,
    actionContext,
    specs,
  });
  const routerVerdict = evaluateRouter({
    session,
    taskGraph,
    sprintStatus,
    validatorVerdict,
    policyVerdict,
    specs,
  });
  const result = {
    validator_verdict: validatorVerdict,
    policy_verdict: policyVerdict,
    scheduler_signals: schedulerSignals,
    ...routerVerdict,
  };
  if (options.syncSprintStatus) {
    synchronizeSprintStatus(rootDir, sprintStatus, taskGraph, routerVerdict);
  }
  return result;
}

module.exports = {
  runEngineKernel,
  validateRuntimeState,
};
