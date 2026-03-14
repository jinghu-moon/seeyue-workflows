#!/usr/bin/env node
"use strict";

const path = require("node:path");

const { createCheckpoint } = require("./checkpoints.cjs");
const { appendEvent } = require("./journal.cjs");
const { getNodeById } = require("./runtime-state.cjs");
const { runEngineKernel } = require("./engine-kernel.cjs");
const {
  listCapsules,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeSession,
  writeTaskGraph,
} = require("./store.cjs");
const { buildReviewActionContext } = require("./verification-evidence.cjs");

const ALLOWED_DECISIONS = new Set(["pass", "concerns", "rework", "fail"]);
const ALLOWED_PERSONAS = new Set(["spec_reviewer", "quality_reviewer"]);

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

function reviewFieldForPersona(persona) {
  return persona === "spec_reviewer" ? "spec_review" : "quality_review";
}

function resolveActiveReviewNode(session, taskGraph, nodeId) {
  const activeNodeId = nodeId || session?.node?.active_id;
  if (!activeNodeId || activeNodeId === "none") {
    throw new Error("no active review node to resolve");
  }
  const node = getNodeById(taskGraph, activeNodeId);
  if (!node) {
    throw new Error(`active review node missing: ${activeNodeId}`);
  }
  if (node.status !== "review") {
    throw new Error(`active node is not in review: ${activeNodeId}`);
  }
  return node;
}

function requireReviewHandoffCapsule(rootDir, persona) {
  const capsules = listCapsules(rootDir);
  const latest = capsules.find((entry) => entry.persona === persona);
  const constraints = Array.isArray(latest?.constraints) ? latest.constraints : [];
  if (!latest || !constraints.includes("review_isolation")) {
    throw new Error(`REVIEW_HANDOFF_REQUIRED missing review handoff capsule for ${persona || "unknown"}`);
  }
}

function resolveReviewVerdict(rootDir, options = {}) {
  const decision = String(options.decision || "").trim().toLowerCase();
  if (!ALLOWED_DECISIONS.has(decision)) {
    throw new Error(`invalid review decision: ${options.decision}`);
  }
  const persona = String(options.persona || "").trim();
  if (!ALLOWED_PERSONAS.has(persona)) {
    throw new Error(`invalid review persona: ${options.persona}`);
  }
  const actor = String(options.actor || persona).trim();
  if (!ALLOWED_PERSONAS.has(actor)) {
    throw new Error(`invalid review actor: ${options.actor}`);
  }

  const currentSession = readSession(rootDir);
  const currentTaskGraph = readTaskGraph(rootDir);
  const currentSprintStatus = readSprintStatus(rootDir);
  if (!currentSession || !currentTaskGraph || !currentSprintStatus) {
    throw new Error("review-resolution requires session.yaml, task-graph.yaml, and sprint-status.yaml");
  }

  const targetNode = resolveActiveReviewNode(currentSession, currentTaskGraph, options.nodeId);
  const activePersona = String(currentSession?.node?.owner_persona || "").trim();
  if (activePersona !== persona) {
    throw new Error(`review persona mismatch: expected ${activePersona || "none"} but got ${persona}`);
  }
  requireReviewHandoffCapsule(rootDir, persona);

  const reviewField = reviewFieldForPersona(persona);
  const currentVerdict = String(targetNode?.review_state?.[reviewField] || "pending").trim();
  if (currentVerdict !== "pending") {
    throw new Error(`review verdict already recorded: ${reviewField}=${currentVerdict}`);
  }

  const session = clone(currentSession);
  const taskGraph = clone(currentTaskGraph);
  const recordedAt = nowIso();

  updateNodeStatus(taskGraph, targetNode.id, (draft) => ({
    ...draft,
    review_state: {
      ...(draft.review_state || {}),
      [reviewField]: decision,
    },
  }));

  session.timestamps = {
    ...session.timestamps,
    updated_at: recordedAt,
  };

  writeSession(rootDir, session);
  writeTaskGraph(rootDir, taskGraph);

  appendEvent(rootDir, {
    runId: session.run_id,
    event: "review_verdict_recorded",
    actor,
    phase: session.phase?.current || targetNode.phase_id || "none",
    nodeId: targetNode.id,
    payload: {
      reviewer_persona: persona,
      decision,
      reason: options.reason || null,
      findings_ref: options.findingsRef || null,
      recorded_at: recordedAt,
    },
  });

  const checkpoint = createCheckpoint(rootDir, {
    checkpointClass: "review",
    phase: session.phase?.current || targetNode.phase_id || "none",
    nodeId: targetNode.id,
    actor,
    sourceEvent: "review_verdict_recorded",
    metadata: {
      reviewer_persona: persona,
      decision,
      reason: options.reason || null,
      findings_ref: options.findingsRef || null,
    },
  });

  const actionContext = buildReviewActionContext(rootDir, options.actionContext || {}, {
    persona,
    decision,
    recordedAt,
    source: "review-resolution",
  });
  const refreshed = runEngineKernel(rootDir, {
    syncSprintStatus: true,
    actionContext,
    specRootDir: rootDir,
  });

  return {
    decision,
    reviewer_persona: persona,
    target_ref: targetNode.id,
    target_node_id: targetNode.id,
    event: "review_verdict_recorded",
    actor,
    checkpoint_id: checkpoint.checkpoint_id,
    route_verdict: refreshed.route_verdict,
    recommended_next: clone(refreshed.recommended_next || []),
    files_updated: [
      ".ai/workflow/session.yaml",
      ".ai/workflow/task-graph.yaml",
      ".ai/workflow/sprint-status.yaml",
      ".ai/workflow/journal.jsonl",
      ".ai/workflow/checkpoints",
      ".ai/workflow/ledger.md",
    ],
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    decision: null,
    persona: null,
    actor: null,
    reason: null,
    findingsRef: null,
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
      case "--persona":
        index += 1;
        parsed.persona = argv[index];
        break;
      case "--actor":
        index += 1;
        parsed.actor = argv[index];
        break;
      case "--reason":
        index += 1;
        parsed.reason = argv[index];
        break;
      case "--findings-ref":
        index += 1;
        parsed.findingsRef = argv[index];
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
    `[review-resolution] 已写回评审结论：${result.decision}`,
    `persona=${result.reviewer_persona}`,
    `target=${result.target_ref}`,
    `next=${next}`,
  ].join(" ");
}

if (require.main === module) {
  try {
    const parsed = parseArgs(process.argv.slice(2));
    const result = resolveReviewVerdict(parsed.rootDir, parsed);
    if (parsed.json) {
      console.log(JSON.stringify(result, null, 2));
    } else {
      console.log(formatHumanSummary(result));
    }
  } catch (error) {
    console.error(`[review-resolution] ${error.message}`);
    process.exit(1);
  }
}

module.exports = {
  resolveReviewVerdict,
  applyReviewDecision: resolveReviewVerdict,
};
