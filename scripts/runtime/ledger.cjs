"use strict";

const { readEvents } = require("./journal.cjs");
const {
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeLedger,
} = require("./store.cjs");

function toArray(value) {
  return Array.isArray(value) ? value : [];
}

function formatScalar(value, fallback = "none") {
  const normalized = value === undefined || value === null || value === "" ? fallback : String(value);
  return normalized;
}

function summarizeRecommendedNext(recommendedNext) {
  const items = toArray(recommendedNext);
  if (items.length === 0) {
    return ["- none"];
  }
  return items.map((entry) => {
    const type = formatScalar(entry?.type);
    const target = formatScalar(entry?.target);
    const reason = formatScalar(entry?.reason, "none");
    return `- ${type}:${target} | reason=${reason}`;
  });
}

function summarizeLatestEvidence(events, activeNodeId) {
  const items = toArray(events);
  const scoped = activeNodeId && activeNodeId !== "none"
    ? items.filter((entry) => entry?.node_id === activeNodeId)
    : items;
  const selected = (scoped.length > 0 ? scoped : items).slice(-3);
  if (selected.length === 0) {
    return ["- none"];
  }
  return selected.map((entry) => {
    const eventName = formatScalar(entry?.event);
    const nodeId = formatScalar(entry?.node_id);
    const actor = formatScalar(entry?.actor);
    return `- ${eventName} | node=${nodeId} | actor=${actor}`;
  });
}

function summarizePendingApproval(session) {
  const approvals = session?.approvals || {};
  const activeRequest = approvals.active_request || null;
  if (approvals.pending !== true || !activeRequest) {
    return [
      "- pending: no",
      `- pending_count: ${Number(approvals.pending_count || 0)}`,
    ];
  }
  return [
    "- pending: yes",
    `- pending_count: ${Number(approvals.pending_count || 0)}`,
    `- target_ref: ${formatScalar(activeRequest.target_ref)}`,
    `- approval_mode: ${formatScalar(activeRequest.approval_mode)}`,
    `- risk_class: ${formatScalar(activeRequest.risk_class)}`,
  ];
}

function listVerifiedNodes(taskGraph) {
  return toArray(taskGraph?.nodes).filter((node) => {
    if (!node || node.status !== "completed") {
      return false;
    }
    if (node.tdd_required === false) {
      return node.tdd_state === "not_applicable" || node.tdd_state === "verified";
    }
    return node.tdd_state === "verified";
  });
}

function buildLedgerDocument(state = {}) {
  const session = state.session || {};
  const taskGraph = state.taskGraph || {};
  const sprintStatus = state.sprintStatus || {};
  const journal = toArray(state.journal);
  const activeNodeId = session?.node?.active_id || "none";
  const verifiedNodes = listVerifiedNodes(taskGraph);

  const lines = [
    "# Current Run",
    "",
    `- run_id: ${formatScalar(session?.run_id)}`,
    `- task_id: ${formatScalar(session?.task?.id)}`,
    `- task_title: ${formatScalar(session?.task?.title)}`,
    `- task_mode: ${formatScalar(session?.task?.mode)}`,
    `- engine_kind: ${formatScalar(session?.engine?.kind)}`,
    "",
    "## Active Phase",
    "",
    `- current: ${formatScalar(session?.phase?.current || sprintStatus?.active_phase)}`,
    `- status: ${formatScalar(session?.phase?.status)}`,
    "",
    "## Active Node",
    "",
    `- id: ${formatScalar(activeNodeId)}`,
    `- owner_persona: ${formatScalar(session?.node?.owner_persona)}`,
    `- state: ${formatScalar(session?.node?.state)}`,
    "",
    "## Latest Evidence",
    "",
    ...summarizeLatestEvidence(journal, activeNodeId),
    "",
    "## Pending Approval",
    "",
    ...summarizePendingApproval(session),
    "",
    "## Recommended Next",
    "",
    ...summarizeRecommendedNext(sprintStatus?.recommended_next),
    "",
    "## Verified Nodes",
    "",
    `- count: ${verifiedNodes.length}`,
  ];

  if (verifiedNodes.length > 0) {
    for (const node of verifiedNodes) {
      lines.push(`### ${formatScalar(node.id)} ✅`);
    }
  } else {
    lines.push("- none");
  }

  lines.push("");
  return `${lines.join("\n")}\n`;
}

function refreshLedger(rootDir) {
  const session = readSession(rootDir);
  const taskGraph = readTaskGraph(rootDir);
  const sprintStatus = readSprintStatus(rootDir);
  if (!session || !taskGraph || !sprintStatus) {
    throw new Error("ledger refresh requires session.yaml, task-graph.yaml, and sprint-status.yaml");
  }
  const content = buildLedgerDocument({
    session,
    taskGraph,
    sprintStatus,
    journal: readEvents(rootDir),
  });
  writeLedger(rootDir, content);
  return content;
}

module.exports = {
  buildLedgerDocument,
  listVerifiedNodes,
  refreshLedger,
};

