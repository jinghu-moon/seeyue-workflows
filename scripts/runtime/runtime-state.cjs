"use strict";

const PRIORITY_ORDER = {
  critical: 0,
  high: 1,
  medium: 2,
  low: 3,
};

function indexById(items) {
  return new Map((Array.isArray(items) ? items : []).map((item) => [item.id, item]));
}

function getPhaseById(taskGraph, phaseId) {
  return indexById(taskGraph?.phases).get(phaseId) || null;
}

function getNodeById(taskGraph, nodeId) {
  return indexById(taskGraph?.nodes).get(nodeId) || null;
}

function getActivePhase(session, taskGraph) {
  return getPhaseById(taskGraph, session?.phase?.current) || null;
}

function getActiveNode(session, taskGraph) {
  if (!session?.node?.active_id || session.node.active_id === "none") {
    return null;
  }
  return getNodeById(taskGraph, session.node.active_id) || null;
}

function listPhaseNodes(taskGraph, phaseId) {
  return (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).filter((node) => node.phase_id === phaseId);
}

function isNodeCompleted(taskGraph, nodeId) {
  return getNodeById(taskGraph, nodeId)?.status === "completed";
}

function areDependenciesCompleted(taskGraph, dependencyIds) {
  return (Array.isArray(dependencyIds) ? dependencyIds : []).every((dependencyId) => isNodeCompleted(taskGraph, dependencyId) || getPhaseById(taskGraph, dependencyId)?.status === "completed");
}

function reviewFailed(reviewState) {
  return [reviewState?.spec_review, reviewState?.quality_review].some((status) => status === "rework" || status === "fail");
}

function reviewAccepted(reviewState) {
  return [reviewState?.spec_review, reviewState?.quality_review].every((status) => status === "pass" || status === "concerns");
}

function phaseIsComplete(taskGraph, phaseId) {
  const nodes = listPhaseNodes(taskGraph, phaseId);
  return nodes.length > 0 && nodes.every((node) => node.status === "completed") && !nodes.some((node) => reviewFailed(node.review_state));
}

function priorityRank(priority) {
  return PRIORITY_ORDER[priority] ?? PRIORITY_ORDER.low;
}

function getPathValue(source, pathExpression) {
  if (!pathExpression) {
    return undefined;
  }
  const segments = pathExpression.split(".");
  let cursor = source;
  for (const segment of segments) {
    if (cursor === null || cursor === undefined || typeof cursor !== "object") {
      return undefined;
    }
    cursor = cursor[segment];
  }
  return cursor;
}

function normalizeLiteral(valueLiteral) {
  if (valueLiteral === "true") {
    return true;
  }
  if (valueLiteral === "false") {
    return false;
  }
  if (valueLiteral === "null") {
    return null;
  }
  if (/^-?\d+$/.test(valueLiteral)) {
    return Number(valueLiteral);
  }
  return valueLiteral.replace(/^['\"]|['\"]$/g, "");
}

function evaluateStateExpression(expression, context) {
  if (!expression || typeof expression !== "string") {
    return true;
  }
  const inMatch = expression.match(/^([a-zA-Z0-9_.]+)\s+in\s+\[(.*)\]$/);
  if (inMatch) {
    const actual = getPathValue(context, inMatch[1]);
    const expected = inMatch[2]
      .split(",")
      .map((item) => item.trim())
      .filter(Boolean)
      .map(normalizeLiteral);
    return expected.includes(actual);
  }
  const equalityMatch = expression.match(/^([a-zA-Z0-9_.]+)\s*==\s*(.+)$/);
  if (equalityMatch) {
    const actual = getPathValue(context, equalityMatch[1]);
    const expected = normalizeLiteral(equalityMatch[2].trim());
    return actual === expected;
  }
  return false;
}

module.exports = {
  areDependenciesCompleted,
  evaluateStateExpression,
  getActiveNode,
  getActivePhase,
  getNodeById,
  getPhaseById,
  indexById,
  listPhaseNodes,
  phaseIsComplete,
  priorityRank,
  reviewAccepted,
  reviewFailed,
};
