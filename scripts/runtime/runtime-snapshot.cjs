"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { readJournalEvents, readSession, readSprintStatus, readTaskGraph } = require("./store.cjs");
const {
  getActiveNode: deriveActiveNode,
  getActivePhase: deriveActivePhase,
} = require("./runtime-state.cjs");

const RUNTIME_SEGMENTS = {
  session: [".ai", "workflow", "session.yaml"],
  taskGraph: [".ai", "workflow", "task-graph.yaml"],
  sprintStatus: [".ai", "workflow", "sprint-status.yaml"],
  journal: [".ai", "workflow", "journal.jsonl"],
};

function isObject(value) {
  return value && typeof value === "object" && !Array.isArray(value);
}

function resolveRuntimePaths(rootDir) {
  const absoluteRoot = path.resolve(rootDir || process.cwd());
  return {
    rootDir: absoluteRoot,
    session: path.join(absoluteRoot, ...RUNTIME_SEGMENTS.session),
    taskGraph: path.join(absoluteRoot, ...RUNTIME_SEGMENTS.taskGraph),
    sprintStatus: path.join(absoluteRoot, ...RUNTIME_SEGMENTS.sprintStatus),
    journal: path.join(absoluteRoot, ...RUNTIME_SEGMENTS.journal),
  };
}

function safeLoad(loader, rootDir) {
  try {
    return { ok: true, value: loader(rootDir), error: null };
  } catch (error) {
    return { ok: false, value: null, error };
  }
}

function isStructuredSession(session) {
  return isObject(session)
    && session.schema === 4
    && isObject(session.engine)
    && isObject(session.task)
    && isObject(session.phase)
    && typeof session.phase.current === "string"
    && isObject(session.node)
    && typeof session.node.active_id === "string"
    && isObject(session.loop_budget)
    && isObject(session.approvals)
    && isObject(session.recovery)
    && isObject(session.timestamps);
}

function isStructuredTaskGraph(taskGraph) {
  return isObject(taskGraph)
    && taskGraph.schema === 4
    && Array.isArray(taskGraph.phases)
    && Array.isArray(taskGraph.nodes);
}

function isStructuredSprintStatus(sprintStatus) {
  return isObject(sprintStatus)
    && sprintStatus.schema === 4
    && Array.isArray(sprintStatus.node_summary)
    && Array.isArray(sprintStatus.recommended_next);
}

function hasCompleteRuntime(snapshot) {
  return Boolean(snapshot && snapshot.complete === true);
}

function getActivePhase(snapshot) {
  return snapshot && snapshot.activePhase ? snapshot.activePhase : null;
}

function getActiveNode(snapshot) {
  return snapshot && snapshot.activeNode ? snapshot.activeNode : null;
}

function getRecommendedNext(snapshot) {
  return Array.isArray(snapshot?.sprintStatus?.recommended_next)
    ? snapshot.sprintStatus.recommended_next
    : [];
}

function isApprovalPending(snapshot) {
  return hasCompleteRuntime(snapshot) && snapshot.session?.approvals?.pending === true;
}

function isRestorePending(snapshot) {
  return hasCompleteRuntime(snapshot) && snapshot.session?.recovery?.restore_pending === true;
}

function isRedVerifiedState(tddState) {
  return [
    "red_verified",
    "green_pending",
    "green_verified",
    "refactor_pending",
    "verified",
  ].includes(String(tddState || "").toLowerCase());
}

function formatRecommendedNext(recommendedNext) {
  const first = Array.isArray(recommendedNext) ? recommendedNext[0] : null;
  if (!first || typeof first !== "object") {
    return "";
  }
  const type = String(first.type || "").trim();
  const target = String(first.target || "").trim();
  if (!type && !target) {
    return "";
  }
  if (!target) {
    return type;
  }
  return `${type}:${target}`;
}

function findLastCompletedNode(snapshot) {
  const journal = Array.isArray(snapshot?.journal) ? snapshot.journal : [];
  for (let index = journal.length - 1; index >= 0; index -= 1) {
    const event = journal[index];
    if (!isObject(event)) {
      continue;
    }
    if (["node_completed", "node_verified", "VERIFY_PASS"].includes(String(event.event || ""))) {
      const nodeId = String(event.node_id || event.node || "").trim();
      if (nodeId) {
        return nodeId;
      }
    }
  }
  const completedNodes = Array.isArray(snapshot?.taskGraph?.nodes)
    ? snapshot.taskGraph.nodes.filter((node) => node && node.status === "completed")
    : [];
  return completedNodes.length > 0 ? String(completedNodes[completedNodes.length - 1].id || "") : "";
}

function projectCompatPhase(snapshot) {
  if (!hasCompleteRuntime(snapshot)) {
    return "";
  }
  const session = snapshot.session || {};
  const activeNode = getActiveNode(snapshot) || {};
  const ownerPersona = String(session?.node?.owner_persona || activeNode.owner_persona || "").toLowerCase();
  const nodeStatus = String(activeNode.status || "").toLowerCase();
  const phaseStatus = String(session?.phase?.status || "").toLowerCase();

  if (phaseStatus === "completed") {
    return "done";
  }
  if (["spec_reviewer", "quality_reviewer", "reader", "auditor"].includes(ownerPersona) || nodeStatus === "review") {
    return "review";
  }
  if (ownerPersona === "planner") {
    return "plan";
  }
  if (String(session?.node?.active_id || "").trim() && String(session.node.active_id).trim().toLowerCase() !== "none") {
    return "execute";
  }
  return "plan";
}

function projectWorkflowCompatState(snapshot) {
  const session = snapshot?.session || {};
  const activeNode = getActiveNode(snapshot) || {};
  const recommendedNext = getRecommendedNext(snapshot);
  const tddState = String(activeNode.tdd_state || session?.node?.state || "");
  const phase = projectCompatPhase(snapshot);

  return {
    phase,
    phaseId: String(session?.phase?.current || "").trim(),
    activeNodeId: String(session?.node?.active_id || "").trim(),
    recommendedNext,
    nextAction: formatRecommendedNext(recommendedNext),
    updatedAt: String(session?.timestamps?.updated_at || "").trim(),
    fields: {
      run_id: String(session?.run_id || "").trim(),
      current_phase: phase,
      phase_id: String(session?.phase?.current || "").trim(),
      next_action: formatRecommendedNext(recommendedNext),
      active_node_id: String(session?.node?.active_id || "").trim(),
      owner_persona: String(session?.node?.owner_persona || activeNode.owner_persona || "").trim(),
      node_state: String(session?.node?.state || activeNode.tdd_state || "").trim(),
      tdd_required: String(Boolean(activeNode.tdd_required)),
      red_verified: String(isRedVerifiedState(tddState)),
      target: String(activeNode.target || "").trim(),
      last_completed_node: findLastCompletedNode(snapshot) || "none",
      total_nodes: String(Array.isArray(snapshot?.taskGraph?.nodes) ? snapshot.taskGraph.nodes.length : ""),
      mode: "",
      loop_budget_max_nodes: String(session?.loop_budget?.max_nodes ?? ""),
      loop_budget_max_consecutive_failures: String(session?.loop_budget?.max_failures ?? ""),
      loop_budget_max_pending_approvals: String(session?.loop_budget?.max_pending_approvals ?? ""),
      updated_at: String(session?.timestamps?.updated_at || "").trim(),
      approval_pending: String(Boolean(session?.approvals?.pending)),
      restore_pending: String(Boolean(session?.recovery?.restore_pending)),
    },
  };
}

function loadRuntimeSnapshot(rootDir = process.cwd()) {
  const paths = resolveRuntimePaths(rootDir);
  const exists = {
    session: fs.existsSync(paths.session),
    taskGraph: fs.existsSync(paths.taskGraph),
    sprintStatus: fs.existsSync(paths.sprintStatus),
    journal: fs.existsSync(paths.journal),
  };

  const sessionResult = exists.session ? safeLoad(readSession, paths.rootDir) : { ok: true, value: null, error: null };
  const taskGraphResult = exists.taskGraph ? safeLoad(readTaskGraph, paths.rootDir) : { ok: true, value: null, error: null };
  const sprintStatusResult = exists.sprintStatus ? safeLoad(readSprintStatus, paths.rootDir) : { ok: true, value: null, error: null };
  const journalResult = exists.journal ? safeLoad(readJournalEvents, paths.rootDir) : { ok: true, value: [], error: null };

  const session = sessionResult.value;
  const taskGraph = taskGraphResult.value;
  const sprintStatus = sprintStatusResult.value;
  const journal = Array.isArray(journalResult.value) ? journalResult.value : [];

  const valid = {
    session: sessionResult.ok && isStructuredSession(session),
    taskGraph: taskGraphResult.ok && isStructuredTaskGraph(taskGraph),
    sprintStatus: sprintStatusResult.ok && isStructuredSprintStatus(sprintStatus),
    journal: journalResult.ok && exists.journal,
  };

  const complete = valid.session && valid.taskGraph && valid.sprintStatus && valid.journal;
  const activePhase = complete ? deriveActivePhase(session, taskGraph) : null;
  const activeNode = complete ? deriveActiveNode(session, taskGraph) : null;

  return {
    rootDir: paths.rootDir,
    paths,
    exists,
    valid,
    complete,
    sourceModel: complete
      ? "v4_runtime"
      : (exists.session || exists.taskGraph || exists.sprintStatus || exists.journal ? "partial_runtime" : "missing"),
    session,
    taskGraph,
    sprintStatus,
    journal,
    activePhase,
    activeNode,
    errors: {
      session: sessionResult.error ? sessionResult.error.message : null,
      taskGraph: taskGraphResult.error ? taskGraphResult.error.message : null,
      sprintStatus: sprintStatusResult.error ? sprintStatusResult.error.message : null,
      journal: journalResult.error ? journalResult.error.message : null,
    },
  };
}

module.exports = {
  getActiveNode,
  getActivePhase,
  getRecommendedNext,
  hasCompleteRuntime,
  isApprovalPending,
  isRestorePending,
  loadRuntimeSnapshot,
  projectCompatPhase,
  projectWorkflowCompatState,
  resolveRuntimePaths,
};
