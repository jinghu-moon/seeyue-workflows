#!/usr/bin/env node
"use strict";

const crypto = require("node:crypto");
const path = require("node:path");

const { buildResumeFrontier } = require("./checkpoints.cjs");
const { readEvents } = require("./journal.cjs");
const { getActiveNode, getActivePhase } = require("./runtime-state.cjs");
const { loadWorkflowSpecs } = require("./workflow-specs.cjs");
const {
  listCapsules,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeCapsule,
} = require("./store.cjs");

const PERSONA_ENUM = new Set(["planner", "author", "spec_reviewer", "quality_reviewer", "reader", "auditor"]);
const DEFAULT_REVIEW_CHAIN = ["author", "spec_reviewer", "quality_reviewer"];

function nowIso() {
  return new Date().toISOString();
}

function buildCapsuleId(persona) {
  return `${persona}-${Date.now()}-${crypto.randomBytes(4).toString("hex")}`;
}

function ensureRuntime(rootDir) {
  const session = readSession(rootDir);
  const taskGraph = readTaskGraph(rootDir);
  const sprintStatus = readSprintStatus(rootDir);
  if (!session || !taskGraph || !sprintStatus) {
    throw new Error("session.yaml, task-graph.yaml, and sprint-status.yaml are required for context manager");
  }
  return { session, taskGraph, sprintStatus };
}

function summarizeRecommendedNext(recommendedNext) {
  const first = Array.isArray(recommendedNext) ? recommendedNext[0] : null;
  if (!first) {
    return "none";
  }
  return `${first.type}:${first.target}`;
}

function lastNodeEventSummary(rootDir, nodeId) {
  const journal = readEvents(rootDir)
    .filter((event) => event.node_id === nodeId)
    .slice(-3);
  if (journal.length === 0) {
    return "No node-local journal evidence yet.";
  }
  return journal
    .map((event) => {
      const payloadKeys = Object.keys(event.payload || {});
      const suffix = payloadKeys.length > 0 ? ` (${payloadKeys.join(",")})` : "";
      return `${event.event}${suffix}`;
    })
    .join(" -> ");
}

function deriveConstraints(session, node, frontier, options = {}) {
  const constraints = [
    "state_over_chat",
    "durable_state_only",
    "progressive_disclosure",
  ];
  if (node?.tdd_required === true) {
    constraints.push("tdd_required");
  }
  if (session?.approvals?.pending === true) {
    constraints.push("approval_pending");
  }
  if (session?.recovery?.restore_pending === true) {
    constraints.push("restore_pending");
  }
  if (frontier?.recommended_next?.length > 0) {
    constraints.push("resume_frontier_present");
  }
  if (options.reviewIsolation === true) {
    constraints.push("review_isolation");
  }
  return constraints;
}

function computeContextBudgetUsed(parts) {
  const joined = parts.filter(Boolean).join("\n");
  return Math.max(1, Math.ceil(joined.length / 64));
}

function deriveVerdict(persona, node) {
  if (persona === "spec_reviewer") {
    return node?.review_state?.spec_review || "pending";
  }
  if (persona === "quality_reviewer") {
    return node?.review_state?.quality_review || "pending";
  }
  if (node?.status === "completed") {
    return "pass";
  }
  return "pending";
}

function buildInputSummary(session, phase, node, frontier, options = {}) {
  if (options.inputSummary) {
    return options.inputSummary;
  }
  return [
    `Task: ${session.task.title} (${session.task.id})`,
    `Phase: ${phase?.id || session.phase.current}`,
    `Node: ${node?.id || session.node.active_id} - ${node?.title || "active node"}`,
    `Action: ${node?.action || "continue execution"}`,
    `Next: ${summarizeRecommendedNext(frontier.recommended_next)}`,
  ].join(" | ");
}

function buildOutputSummary(rootDir, node, frontier, options = {}) {
  if (options.outputSummary) {
    return options.outputSummary;
  }
  return [
    `Evidence: ${lastNodeEventSummary(rootDir, node?.id || "none")}`,
    `Resume frontier: ${summarizeRecommendedNext(frontier.recommended_next)}`,
  ].join(" | ");
}

function createCapsule(rootDir, options = {}) {
  const { session, taskGraph } = ensureRuntime(rootDir);
  const frontier = buildResumeFrontier(rootDir);
  const activeNode = getActiveNode(session, taskGraph);
  const activePhase = getActivePhase(session, taskGraph);
  const persona = String(options.persona || session.node.owner_persona || "author").trim();
  if (!PERSONA_ENUM.has(persona)) {
    throw new Error(`Unsupported capsule persona: ${persona}`);
  }
  const evidenceRefs = Array.isArray(options.evidenceRefs)
    ? options.evidenceRefs
    : Array.isArray(activeNode?.evidence_refs)
      ? activeNode.evidence_refs
      : [];
  const capsule = {
    capsule_id: options.capsuleId || buildCapsuleId(persona),
    persona,
    task_id: session.task.id,
    input_summary: buildInputSummary(session, activePhase, activeNode, frontier, options),
    constraints: deriveConstraints(session, activeNode, frontier, options),
    evidence_refs: evidenceRefs,
    output_summary: buildOutputSummary(rootDir, activeNode, frontier, options),
    verdict: options.verdict || deriveVerdict(persona, activeNode),
    context_budget_used: options.contextBudgetUsed || computeContextBudgetUsed([
      session.task.title,
      activeNode?.title,
      summarizeRecommendedNext(frontier.recommended_next),
      evidenceRefs.join(","),
      options.inputSummary,
      options.outputSummary,
    ]),
    created_at: nowIso(),
  };
  writeCapsule(rootDir, capsule);
  return capsule;
}

function getLatestCapsule(rootDir, options = {}) {
  const persona = options.persona ? String(options.persona).trim() : null;
  const capsules = listCapsules(rootDir);
  if (!persona) {
    return capsules[0] || null;
  }
  return capsules.find((entry) => entry.persona === persona) || null;
}

function getNextReviewerPersona(rootDir, currentPersona, options = {}) {
  if (options.targetPersona) {
    return options.targetPersona;
  }
  let reviewChain = DEFAULT_REVIEW_CHAIN;
  try {
    const specs = loadWorkflowSpecs(rootDir);
    if (Array.isArray(specs?.personaBindings?.review_chain) && specs.personaBindings.review_chain.length > 0) {
      reviewChain = specs.personaBindings.review_chain;
    }
  } catch (_error) {
    reviewChain = DEFAULT_REVIEW_CHAIN;
  }
  const currentIndex = reviewChain.indexOf(currentPersona);
  if (currentIndex < 0 || currentIndex >= reviewChain.length - 1) {
    return "spec_reviewer";
  }
  return reviewChain[currentIndex + 1];
}

function buildReviewHandoffCapsule(rootDir, options = {}) {
  const sourceCapsule = options.sourceCapsule || getLatestCapsule(rootDir, { persona: options.sourcePersona || "author" });
  if (!sourceCapsule) {
    throw new Error("CONTEXT_MANAGER_MISSING_SOURCE_CAPSULE");
  }
  const targetPersona = getNextReviewerPersona(rootDir, sourceCapsule.persona, options);
  return createCapsule(rootDir, {
    persona: targetPersona,
    evidenceRefs: sourceCapsule.evidence_refs,
    inputSummary: `Review handoff from ${sourceCapsule.persona}: ${sourceCapsule.output_summary}`,
    outputSummary: `Await ${targetPersona} review for task ${sourceCapsule.task_id}`,
    verdict: "pending",
    reviewIsolation: true,
  });
}

function evaluateCompactionNeed(rootDir, options = {}) {
  const { session } = ensureRuntime(rootDir);
  const reasons = [];
  const contextUtilization = Number(options.contextUtilization || 0);
  const turnsSinceSummary = Number(options.turnsSinceSummary || 0);
  const turnsSinceCapsule = Number(options.turnsSinceCapsule || 0);
  if (options.force === true) {
    reasons.push("forced");
  }
  if (contextUtilization >= 0.8) {
    reasons.push("context_utilization_high");
  }
  if (turnsSinceSummary >= Number(session.context_budget.summary_required_after_turns || 0)) {
    reasons.push("summary_refresh_due");
  }
  if (turnsSinceCapsule >= Number(session.context_budget.capsule_refresh_threshold || 0)) {
    reasons.push("capsule_refresh_due");
  }
  return {
    required: reasons.length > 0,
    reasons,
    context_utilization: contextUtilization,
  };
}

function compactContext(rootDir, options = {}) {
  const frontier = buildResumeFrontier(rootDir);
  const need = evaluateCompactionNeed(rootDir, options);
  if (!need.required) {
    return {
      compacted: false,
      reasons: need.reasons,
      resume_frontier: frontier,
      latest_capsule: getLatestCapsule(rootDir),
    };
  }
  if (!Array.isArray(frontier.recommended_next) || frontier.recommended_next.length === 0) {
    throw new Error("CONTEXT_MANAGER_RESUME_FRONTIER_LOST");
  }
  const activeCapsule = createCapsule(rootDir, {
    contextBudgetUsed: options.contextBudgetUsed,
  });
  const reviewHandoffCapsule = options.createReviewHandoff === true
    ? buildReviewHandoffCapsule(rootDir, { sourceCapsule: activeCapsule, targetPersona: options.reviewPersona })
    : null;
  return {
    compacted: true,
    reasons: need.reasons,
    active_capsule: activeCapsule,
    review_handoff_capsule: reviewHandoffCapsule,
    resume_frontier: frontier,
    hot_context: {
      capsule_id: activeCapsule.capsule_id,
      evidence_refs: activeCapsule.evidence_refs,
      recommended_next: frontier.recommended_next,
    },
    warm_context: {
      task_id: activeCapsule.task_id,
      verdict: activeCapsule.verdict,
      constraints: activeCapsule.constraints,
    },
    cold_context: {
      journal_ref: ".ai/workflow/journal.jsonl",
      checkpoints_dir: ".ai/workflow/checkpoints",
      capsules_dir: ".ai/workflow/capsules",
    },
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    contextUtilization: 0,
    turnsSinceSummary: 0,
    turnsSinceCapsule: 0,
    createReviewHandoff: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--context-utilization":
        index += 1;
        parsed.contextUtilization = Number(argv[index]);
        break;
      case "--turns-since-summary":
        index += 1;
        parsed.turnsSinceSummary = Number(argv[index]);
        break;
      case "--turns-since-capsule":
        index += 1;
        parsed.turnsSinceCapsule = Number(argv[index]);
        break;
      case "--review-handoff":
        parsed.createReviewHandoff = true;
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }
  return parsed;
}

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }
  try {
    const result = compactContext(parsed.rootDir, parsed);
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  buildReviewHandoffCapsule,
  compactContext,
  createCapsule,
  evaluateCompactionNeed,
  getLatestCapsule,
};

if (require.main === module) {
  main();
}
