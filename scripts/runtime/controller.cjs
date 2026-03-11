#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { resolvePendingApprovalState } = require("./approval-resolution.cjs");
const { recoverInterruptedRun } = require("./checkpoints.cjs");
const { runEngineKernel } = require("./engine-kernel.cjs");
const { writeReport } = require("./report-builder.cjs");
const { resolveReviewVerdict } = require("./review-resolution.cjs");
const { readSession } = require("./store.cjs");
const { applyRuntimeTransition } = require("./transition-applier.cjs");
const { applyRuntimeStateRepair } = require("./state-repair.cjs");
const { buildReviewActionContext, buildVerifyActionContext } = require("./verification-evidence.cjs");

const REPORT_RELATIVE_PATH = ".ai/analysis/ai.report.json";
const ALLOWED_MODES = new Set(["run", "resume", "verify"]);
const REVIEW_PERSONAS = new Set(["spec_reviewer", "quality_reviewer"]);

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function summarizeRecommendedNext(decision) {
  const first = Array.isArray(decision?.recommended_next) ? decision.recommended_next[0] : null;
  if (!first) {
    return "none";
  }
  return `${first.type || "unknown"}:${first.target || "none"}`;
}

function mergeFilesUpdated(...groups) {
  const seen = new Set();
  const merged = [];
  for (const group of groups) {
    if (!Array.isArray(group)) {
      continue;
    }
    for (const item of group) {
      if (typeof item !== "string" || seen.has(item)) {
        continue;
      }
      seen.add(item);
      merged.push(item);
    }
  }
  return merged;
}

function loadVerificationReport(rootDir) {
  const reportPath = path.join(rootDir, REPORT_RELATIVE_PATH);
  if (!fs.existsSync(reportPath)) {
    return {
      report_exists: false,
      report_ref: null,
      report_overall: null,
    };
  }
  const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
  return {
    report_exists: true,
    report_ref: REPORT_RELATIVE_PATH,
    report_overall: report?.overall || null,
  };
}

function buildVerificationStatus(rootDir, decision) {
  const session = readSession(rootDir);
  const report = loadVerificationReport(rootDir);
  const phaseStatus = session?.phase?.status || null;
  const reportReady = report.report_overall === "READY";
  const terminalHandoffReady = phaseStatus === "completed" && reportReady;

  return {
    phase_status: phaseStatus,
    phase_current: session?.phase?.current || null,
    active_node: session?.node?.active_id || null,
    review_ready: ["review", "completed"].includes(phaseStatus || "") && reportReady,
    terminal_handoff_ready: terminalHandoffReady,
    session_stopped: terminalHandoffReady && session?.node?.active_id === "none",
    ...report,
    recommended_next: clone(decision?.recommended_next || []),
  };
}

function buildApprovalDecisionSnapshot(rootDir, approvalResult) {
  const session = readSession(rootDir);
  const recommendedNext = clone(approvalResult?.recommended_next || []);
  const first = Array.isArray(recommendedNext) ? recommendedNext[0] : null;
  return {
    route_verdict: approvalResult?.route_verdict || "hold",
    active_phase: session?.phase?.current || null,
    active_node: session?.node?.active_id || null,
    next_capability: first?.type || null,
    block_reason: first?.type === "human_intervention" ? "human_decision_required" : null,
    recommended_next: recommendedNext,
  };
}

function buildReviewDecisionSnapshot(rootDir, reviewResult) {
  const session = readSession(rootDir);
  const recommendedNext = clone(reviewResult?.recommended_next || []);
  const first = Array.isArray(recommendedNext) ? recommendedNext[0] : null;
  return {
    route_verdict: reviewResult?.route_verdict || "hold",
    active_phase: session?.phase?.current || null,
    active_node: session?.node?.active_id || null,
    next_capability: first?.type || null,
    block_reason: first?.type === "human_intervention" ? "human_decision_required" : null,
    recommended_next: recommendedNext,
  };
}

function firstRecommendedAction(decision) {
  return Array.isArray(decision?.recommended_next) ? decision.recommended_next[0] || null : null;
}

function normalizeMaxHops(value, fallback = 8) {
  if (value === null || value === undefined) {
    return fallback;
  }
  const parsed = Number(value);
  if (!Number.isFinite(parsed) || parsed < 0) {
    return fallback;
  }
  return Math.floor(parsed);
}

function resolveLoopConfig(options = {}) {
  const enabled = options.autoLoop === true || options.autoAdvance === true;
  return {
    enabled,
    max_hops: normalizeMaxHops(options.maxHops, 8),
  };
}

function remainingBudget(limit, consumed) {
  const safeLimit = Number(limit);
  const safeConsumed = Number(consumed);
  if (!Number.isFinite(safeLimit)) {
    return null;
  }
  if (!Number.isFinite(safeConsumed)) {
    return safeLimit;
  }
  return safeLimit - safeConsumed;
}

function buildLoopBudgetSnapshot(session) {
  const loopBudget = session?.loop_budget || {};
  return {
    max_nodes: Number(loopBudget.max_nodes || 0),
    consumed_nodes: Number(loopBudget.consumed_nodes || 0),
    remaining_nodes: remainingBudget(loopBudget.max_nodes, loopBudget.consumed_nodes),
    max_failures: Number(loopBudget.max_failures || 0),
    consumed_failures: Number(loopBudget.consumed_failures || 0),
    remaining_failures: remainingBudget(loopBudget.max_failures, loopBudget.consumed_failures),
    max_pending_approvals: Number(loopBudget.max_pending_approvals || 0),
    pending_approvals: Number(session?.approvals?.pending_count || 0),
    remaining_approval_slots: remainingBudget(loopBudget.max_pending_approvals, session?.approvals?.pending_count || 0),
  };
}

function detectLoopBudgetStopReason(session) {
  const budget = buildLoopBudgetSnapshot(session);
  if (budget.remaining_nodes !== null && budget.remaining_nodes <= 0) {
    return "node_budget_exhausted";
  }
  if (budget.remaining_failures !== null && budget.remaining_failures <= 0) {
    return "failure_budget_exhausted";
  }
  if (budget.remaining_approval_slots !== null && budget.remaining_approval_slots <= 0) {
    return "approval_budget_exhausted";
  }
  return null;
}

function buildLoopTraceEntry(decision) {
  const nextAction = firstRecommendedAction(decision);
  return {
    route_verdict: decision?.route_verdict || null,
    active_phase: decision?.active_phase || null,
    active_node: decision?.active_node || null,
    next_action_type: nextAction?.type || null,
    next_action_target: nextAction?.target || null,
    block_reason: decision?.block_reason || null,
  };
}

function readLoopStateSnapshot(rootDir) {
  return {
    session: readSession(rootDir),
    taskGraph: null,
    sprintStatus: null,
    filesUpdated: [],
    appendedEvents: [],
    createdCheckpoints: [],
  };
}

function isAutoLoopableActionType(actionType) {
  return ["enter_phase", "start_node", "request_approval", "human_intervention"].includes(String(actionType || ""));
}

function isReviewHandoffDecision(decision) {
  const nextAction = firstRecommendedAction(decision);
  return Boolean(
    decision?.route_verdict === "handoff"
      && nextAction?.type === "resume_node"
      && REVIEW_PERSONAS.has(String(decision?.next_persona || "")),
  );
}

function shouldAutoApplyFollowUpDecision(loopConfig, decision) {
  if (!loopConfig.enabled) {
    return false;
  }
  const nextAction = firstRecommendedAction(decision);
  if (!nextAction) {
    return false;
  }
  if (isReviewHandoffDecision(decision)) {
    return true;
  }
  return isAutoLoopableActionType(nextAction.type);
}

function classifyFrontier(session) {
  const activeNodeId = session?.node?.active_id || "none";
  const ownerPersona = String(session?.node?.owner_persona || "").trim();
  if (activeNodeId === "none") {
    if (session?.approvals?.pending === true) {
      return { kind: "approval", stop_reason: "approval_pending" };
    }
    if (session?.recovery?.restore_pending === true) {
      return { kind: "recovery", stop_reason: "restore_pending" };
    }
    return { kind: "none", stop_reason: null };
  }
  if (REVIEW_PERSONAS.has(ownerPersona)) {
    return { kind: "review", stop_reason: "review_frontier_reached" };
  }
  if (ownerPersona === "author") {
    return { kind: "author", stop_reason: "author_frontier_reached" };
  }
  if (ownerPersona === "human") {
    return { kind: "human", stop_reason: "human_frontier_reached" };
  }
  return { kind: ownerPersona || "execution", stop_reason: "execution_frontier_reached" };
}

function evaluateLoopContinuation(mode, decision, applyResult, loopConfig, hopCount) {
  if (!loopConfig.enabled) {
    return { continue_loop: false, stop_reason: "auto_loop_disabled" };
  }

  if (hopCount >= loopConfig.max_hops) {
    return { continue_loop: false, stop_reason: "max_hops_reached" };
  }

  const session = applyResult?.session;
  if (!session) {
    return { continue_loop: false, stop_reason: "session_missing" };
  }

  const budgetStopReason = detectLoopBudgetStopReason(session);
  if (budgetStopReason) {
    return { continue_loop: false, stop_reason: budgetStopReason };
  }

  if (session.approvals?.pending === true) {
    return { continue_loop: false, stop_reason: "approval_pending" };
  }

  if (session.recovery?.restore_pending === true) {
    return { continue_loop: false, stop_reason: "restore_pending" };
  }

  if (session.phase?.status === "completed") {
    return { continue_loop: false, stop_reason: "phase_completed" };
  }

  if ((session.node?.active_id || "none") !== "none") {
    const frontier = classifyFrontier(session);
    return { continue_loop: false, stop_reason: frontier.stop_reason, frontier_kind: frontier.kind };
  }

  const nextAction = firstRecommendedAction(decision);
  if (!nextAction) {
    return { continue_loop: false, stop_reason: "no_recommended_next" };
  }

  if (nextAction.type === "enter_phase") {
    return { continue_loop: true, stop_reason: null };
  }

  if (mode === "verify" && nextAction.type === "start_node") {
      return { continue_loop: false, stop_reason: "author_frontier_reached", frontier_kind: "author" };
  }

  return { continue_loop: false, stop_reason: "no_loopable_transition" };
}

function mergeApplyResults(previousResult, nextResult) {
  return {
    session: nextResult.session,
    taskGraph: nextResult.taskGraph,
    sprintStatus: nextResult.sprintStatus,
    filesUpdated: mergeFilesUpdated(previousResult?.filesUpdated, nextResult?.filesUpdated),
    appendedEvents: [
      ...(Array.isArray(previousResult?.appendedEvents) ? previousResult.appendedEvents : []),
      ...(Array.isArray(nextResult?.appendedEvents) ? nextResult.appendedEvents : []),
    ],
    createdCheckpoints: [
      ...(Array.isArray(previousResult?.createdCheckpoints) ? previousResult.createdCheckpoints : []),
      ...(Array.isArray(nextResult?.createdCheckpoints) ? nextResult.createdCheckpoints : []),
    ],
  };
}

function buildLoopSummary(loopConfig, applyResult, decisionChain, stopReason) {
  const frontier = classifyFrontier(applyResult?.session || null);
  return {
    enabled: loopConfig.enabled,
    max_hops: loopConfig.max_hops,
    hops_executed: Math.max(0, decisionChain.length - 1),
    stop_reason: stopReason,
    frontier_kind: frontier.kind,
    budget: buildLoopBudgetSnapshot(applyResult?.session || null),
    trace: decisionChain.map((entry) => buildLoopTraceEntry(entry)),
  };
}

function executeControllerLoop(rootDir, options) {
  const {
    mode,
    actionContext,
    loopConfig,
    initialDecision,
    initialApplyMode,
    initialApplyOptions,
    initialDecisionChain,
  } = options;

  let applyResult = initialDecision
    ? applyRuntimeTransition(rootDir, {
        mode: initialApplyMode,
        decision: initialDecision,
        actionContext,
        ...(initialApplyOptions || {}),
      })
    : readLoopStateSnapshot(rootDir);
  let finalDecision = initialDecision || null;
  const decisionChain = Array.isArray(initialDecisionChain) ? [...initialDecisionChain] : [];
  if (initialDecision) {
    decisionChain.push(clone(initialDecision));
  }
  let hopCount = 0;
  let loopState = initialDecision
    ? evaluateLoopContinuation(mode, finalDecision, applyResult, loopConfig, hopCount)
    : { continue_loop: loopConfig.enabled, stop_reason: loopConfig.enabled ? null : "auto_loop_disabled" };

  while (loopState.continue_loop === true) {
    const followUpDecision = runEngineKernel(rootDir, {
      syncSprintStatus: false,
      actionContext,
      specRootDir: options.specRootDir || rootDir,
    });
    decisionChain.push(clone(followUpDecision));
    finalDecision = followUpDecision;
    if (!shouldAutoApplyFollowUpDecision(loopConfig, followUpDecision)) {
      loopState = { continue_loop: false, stop_reason: "decision_not_auto_loopable" };
      break;
    }
    const followUpApply = applyRuntimeTransition(rootDir, {
      mode: "run",
      decision: followUpDecision,
      actionContext,
    });
    applyResult = mergeApplyResults(applyResult, followUpApply);
    hopCount += 1;
    loopState = evaluateLoopContinuation(mode, finalDecision, applyResult, loopConfig, hopCount);
  }

  return {
    finalDecision,
    decisionChain,
    applyResult,
    loopState,
  };
}

function runController(rootDir, options = {}) {
  const repairResult = options.repairState
    ? applyRuntimeStateRepair(rootDir, { specRootDir: options.specRootDir || rootDir })
    : null;
  const loopConfig = resolveLoopConfig(options);

  if (options.reviewDecision) {
    const reviewActionContext = buildReviewActionContext(rootDir, options.actionContext || {}, {
      persona: options.reviewPersona,
      decision: options.reviewDecision,
      source: "controller-review-flow",
    });
    const review = resolveReviewVerdict(rootDir, {
      decision: options.reviewDecision,
      persona: options.reviewPersona,
      actor: options.reviewActor || options.reviewPersona,
      reason: options.reviewReason,
      findingsRef: options.reviewFindingsRef,
      actionContext: reviewActionContext,
    });
    const reviewDecision = buildReviewDecisionSnapshot(rootDir, review);

    if (!loopConfig.enabled) {
      return {
        mode: "review",
        root_dir: rootDir.replace(/\\/g, "/"),
        repair: repairResult,
        review,
        decision: reviewDecision,
        decision_chain: [reviewDecision],
        loop_summary: {
          enabled: loopConfig.enabled,
          max_hops: loopConfig.max_hops,
          hops_executed: 0,
          stop_reason: "auto_loop_disabled",
          budget: buildLoopBudgetSnapshot(readSession(rootDir)),
          trace: [buildLoopTraceEntry(reviewDecision)],
        },
        files_updated: mergeFilesUpdated(repairResult?.files_updated, review.files_updated),
        verification: null,
      };
    }

    const loopExecution = executeControllerLoop(rootDir, {
      mode: "review",
      actionContext: reviewActionContext,
      loopConfig,
      initialDecision: null,
      initialApplyMode: null,
      initialApplyOptions: null,
      initialDecisionChain: [reviewDecision],
      specRootDir: options.specRootDir || rootDir,
    });

    return {
      mode: "review",
      root_dir: rootDir.replace(/\\/g, "/"),
      repair: repairResult,
      review,
      decision: loopExecution.finalDecision,
      decision_chain: loopExecution.decisionChain,
      loop_summary: buildLoopSummary(loopConfig, loopExecution.applyResult, loopExecution.decisionChain, loopExecution.loopState.stop_reason),
      files_updated: mergeFilesUpdated(repairResult?.files_updated, review.files_updated, loopExecution.applyResult.filesUpdated),
      verification: null,
    };
  }

  if (options.approvalDecision) {
    const approval = resolvePendingApprovalState(rootDir, {
      decision: options.approvalDecision,
      approvalId: options.approvalId,
      actor: options.approvalActor,
      reason: options.approvalReason,
      expiresAt: options.approvalExpiresAt,
    });
    const approvalDecision = buildApprovalDecisionSnapshot(rootDir, approval);

    if (!(loopConfig.enabled && approval.decision === "approved")) {
      return {
        mode: "approval",
        root_dir: rootDir.replace(/\\/g, "/"),
        repair: repairResult,
        approval,
        decision: approvalDecision,
        decision_chain: [approvalDecision],
        loop_summary: {
          enabled: loopConfig.enabled,
          max_hops: loopConfig.max_hops,
          hops_executed: 0,
          stop_reason: loopConfig.enabled ? "approval_flow_terminal" : "auto_loop_disabled",
          budget: buildLoopBudgetSnapshot(readSession(rootDir)),
          trace: [buildLoopTraceEntry(approvalDecision)],
        },
        files_updated: mergeFilesUpdated(repairResult?.files_updated, approval.files_updated),
        verification: null,
      };
    }

    const loopExecution = executeControllerLoop(rootDir, {
      mode: "approval",
      actionContext: options.actionContext || {},
      loopConfig,
      initialDecision: null,
      initialApplyMode: null,
      initialApplyOptions: null,
      initialDecisionChain: [approvalDecision],
      specRootDir: options.specRootDir || rootDir,
    });

    return {
      mode: "approval",
      root_dir: rootDir.replace(/\\/g, "/"),
      repair: repairResult,
      approval,
      decision: loopExecution.finalDecision,
      decision_chain: loopExecution.decisionChain,
      loop_summary: buildLoopSummary(loopConfig, loopExecution.applyResult, loopExecution.decisionChain, loopExecution.loopState.stop_reason),
      files_updated: mergeFilesUpdated(repairResult?.files_updated, approval.files_updated, loopExecution.applyResult.filesUpdated),
      verification: null,
    };
  }

  const mode = options.mode || "run";
  const writtenReport = mode === "verify" && options.writeReport ? writeReport(rootDir) : null;
  const recovery = mode === "resume"
    ? recoverInterruptedRun(rootDir, { actor: "runtime" })
    : null;
  const actionContext = mode === "verify"
    ? buildVerifyActionContext(rootDir, options.actionContext || {})
    : (options.actionContext || {});

  const decision = runEngineKernel(rootDir, {
    syncSprintStatus: false,
    actionContext,
    specRootDir: options.specRootDir || rootDir,
  });

  const loopExecution = executeControllerLoop(rootDir, {
    mode,
    actionContext,
    loopConfig,
    initialDecision: decision,
    initialApplyMode: mode,
    initialApplyOptions: {
      allowVerifyTransition: loopConfig.enabled && mode === "verify",
    },
    initialDecisionChain: [],
    specRootDir: options.specRootDir || rootDir,
  });

  return {
    mode,
      root_dir: rootDir.replace(/\\/g, "/"),
      repair: repairResult,
      recovery,
      approval: null,
      decision: loopExecution.finalDecision,
      decision_chain: loopExecution.decisionChain,
      loop_summary: buildLoopSummary(loopConfig, loopExecution.applyResult, loopExecution.decisionChain, loopExecution.loopState.stop_reason),
      files_updated: mergeFilesUpdated(repairResult?.files_updated, loopExecution.applyResult.filesUpdated),
    verification: mode === "verify"
      ? {
          ...buildVerificationStatus(rootDir, loopExecution.finalDecision),
          report_written: Boolean(writtenReport),
        }
      : null,
  };
}

function formatHumanSummary(result) {
  if (result.mode === "approval") {
    return [
      "[sy-controller] 审批结果已写回。",
      `decision=${result.approval?.decision || "unknown"}`,
      `approval_id=${result.approval?.approval_id || "none"}`,
      `phase=${result.decision?.active_phase || "none"}`,
      `node=${result.decision?.active_node || "none"}`,
      `next=${summarizeRecommendedNext(result.decision)}`,
    ].join(" ");
  }

  if (result.mode === "verify") {
    const verification = result.verification || {};
    const repairSummary = result.repair?.applied
      ? `运行态修复：已应用 ${result.repair.repair_count} 项`
      : "运行态修复：无";
    return [
      "[sy-controller] 验证模式已执行。",
      `当前阶段：${verification.phase_current || "unknown"} / ${verification.phase_status || "unknown"}`,
      repairSummary,
      `验证报告：${verification.report_overall || "missing"}`,
      `是否可进入人工审批：${verification.review_ready ? "是" : "否"}`,
      `是否已进入终态交接：${verification.terminal_handoff_ready ? "是" : "否"}`,
    ].join(" ");
  }

  return [
    `[sy-controller] ${result.mode === "resume" ? "恢复" : "执行"}模式已执行。`,
    `repair=${result.repair?.applied ? result.repair.repair_count : 0}`,
    `route=${result.decision.route_verdict}`,
    `phase=${result.decision.active_phase || "none"}`,
    `node=${result.decision.active_node || "none"}`,
    `next=${summarizeRecommendedNext(result.decision)}`,
  ].join(" ");
}

function formatHumanSummaryZh(result) {
  if (result.mode === "approval") {
    return [
      "[sy-controller] 审批结果已写回。",
      `decision=${result.approval?.decision || "unknown"}`,
      `approval_id=${result.approval?.approval_id || "none"}`,
      `phase=${result.decision?.active_phase || "none"}`,
      `node=${result.decision?.active_node || "none"}`,
      `next=${summarizeRecommendedNext(result.decision)}`,
    ].join(" ");
  }

  if (result.mode === "review") {
    return [
      "[sy-controller] 评审结论已写回。",
      `decision=${result.review?.decision || "unknown"}`,
      `persona=${result.review?.reviewer_persona || "unknown"}`,
      `phase=${result.decision?.active_phase || "none"}`,
      `node=${result.decision?.active_node || "none"}`,
      `next=${summarizeRecommendedNext(result.decision)}`,
    ].join(" ");
  }

  if (result.mode === "verify") {
    const verification = result.verification || {};
    const repairSummary = result.repair?.applied
      ? `运行态修复：已应用 ${result.repair.repair_count} 项`
      : "运行态修复：无";
    return [
      "[sy-controller] 验证模式已执行。",
      `当前阶段：${verification.phase_current || "unknown"} / ${verification.phase_status || "unknown"}`,
      repairSummary,
      `验证报告：${verification.report_overall || "missing"}`,
      `是否可进入人工审批：${verification.review_ready ? "是" : "否"}`,
      `是否已进入终态交接：${verification.terminal_handoff_ready ? "是" : "否"}`,
    ].join(" ");
  }

  return [
    `[sy-controller] ${result.mode === "resume" ? "恢复" : "执行"}模式已执行。`,
    `repair=${result.repair?.applied ? result.repair.repair_count : 0}`,
    `route=${result.decision.route_verdict}`,
    `phase=${result.decision.active_phase || "none"}`,
    `node=${result.decision.active_node || "none"}`,
    `next=${summarizeRecommendedNext(result.decision)}`,
  ].join(" ");
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    mode: null,
    approvalDecision: null,
    approvalId: null,
    approvalActor: "human",
    approvalReason: null,
    approvalExpiresAt: null,
    reviewDecision: null,
    reviewPersona: null,
    reviewActor: null,
    reviewReason: null,
    reviewFindingsRef: null,
    json: false,
    writeReport: false,
    autoLoop: false,
    autoAdvance: false,
    maxHops: null,
    repairState: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--mode":
        index += 1;
        parsed.mode = argv[index];
        break;
      case "--approval-decision":
        index += 1;
        parsed.approvalDecision = argv[index];
        break;
      case "--approval-id":
        index += 1;
        parsed.approvalId = argv[index];
        break;
      case "--approval-actor":
        index += 1;
        parsed.approvalActor = argv[index];
        break;
      case "--approval-reason":
        index += 1;
        parsed.approvalReason = argv[index];
        break;
      case "--approval-expires-at":
        index += 1;
        parsed.approvalExpiresAt = argv[index];
        break;
      case "--review-decision":
        index += 1;
        parsed.reviewDecision = argv[index];
        break;
      case "--review-persona":
        index += 1;
        parsed.reviewPersona = argv[index];
        break;
      case "--review-actor":
        index += 1;
        parsed.reviewActor = argv[index];
        break;
      case "--review-reason":
        index += 1;
        parsed.reviewReason = argv[index];
        break;
      case "--review-findings-ref":
        index += 1;
        parsed.reviewFindingsRef = argv[index];
        break;
      case "--json":
        parsed.json = true;
        break;
      case "--write-report":
        parsed.writeReport = true;
        break;
      case "--auto-loop":
        parsed.autoLoop = true;
        break;
      case "--auto-advance":
        parsed.autoAdvance = true;
        break;
      case "--max-hops":
        index += 1;
        parsed.maxHops = argv[index];
        break;
      case "--repair-state":
        parsed.repairState = true;
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }

  if (parsed.approvalDecision && parsed.reviewDecision) {
    throw new Error("Do not combine --approval-decision with --review-decision");
  }
  if ((parsed.approvalDecision || parsed.reviewDecision) && parsed.mode) {
    throw new Error("Do not combine --mode with approval/review decision flows; controller refreshes routing directly");
  }
  if (!parsed.approvalDecision && !parsed.reviewDecision && (!parsed.mode || !ALLOWED_MODES.has(parsed.mode))) {
    throw new Error("Missing or invalid --mode. Expected run|resume|verify");
  }
  if (parsed.autoAdvance && parsed.mode !== "verify") {
    throw new Error("--auto-advance is only valid with --mode verify");
  }
  if (parsed.maxHops !== null && (!/^\d+$/.test(String(parsed.maxHops)) || Number(parsed.maxHops) < 0)) {
    throw new Error("--max-hops must be a non-negative integer");
  }
  if (parsed.approvalDecision && !parsed.approvalId) {
    throw new Error("Missing --approval-id for approval flow");
  }
  if (parsed.reviewDecision && !parsed.reviewPersona) {
    throw new Error("Missing --review-persona for review flow");
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
    const result = runController(parsed.rootDir, parsed);
    if (parsed.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }
    console.log(formatHumanSummaryZh(result));
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  buildApprovalDecisionSnapshot,
  buildVerificationStatus,
  formatHumanSummary,
  formatHumanSummaryZh,
  mergeFilesUpdated,
  parseArgs,
  runController,
};

if (require.main === module) {
  main();
}
