"use strict";

const {
  COVERAGE_TARGETS,
  validateCoverageEvidenceEnvelope,
} = require("./coverage-adapter.cjs");
const {
  getActiveNode,
  getActivePhase,
  listPhaseNodes,
  reviewAccepted,
  reviewFailed,
} = require("./runtime-state.cjs");

const RISK_ORDER = {
  low: 0,
  medium: 1,
  high: 2,
  critical: 3,
};

function toPosixPath(targetPath) {
  return String(targetPath || "").replace(/\\/g, "/");
}

function globToRegExp(pattern) {
  const normalized = toPosixPath(pattern);
  const segments = normalized.split("/");
  let expression = "^";

  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    if (segment === "**") {
      expression += "(?:[^/]+/)*";
      continue;
    }

    const escaped = segment
      .replace(/[.+^${}()|[\]\\]/g, "\\$&")
      .replace(/\*/g, "[^/]*")
      .replace(/\?/g, "[^/]");
    expression += escaped;

    if (index < segments.length - 1) {
      expression += "/";
    }
  }

  expression += "$";
  return new RegExp(expression);
}

function classifyFilePath(fileClassesSpec, targetPath) {
  const normalizedPath = toPosixPath(targetPath);
  const classes = fileClassesSpec?.classes || {};
  const precedence = Array.isArray(fileClassesSpec?.match_precedence) ? fileClassesSpec.match_precedence : Object.keys(classes);
  for (const classId of precedence) {
    const entry = classes[classId];
    if (!entry || !Array.isArray(entry.patterns)) {
      continue;
    }
    if (entry.patterns.some((pattern) => globToRegExp(pattern).test(normalizedPath))) {
      return classId;
    }
  }
  return null;
}

function normalizeCommandClass(approvalMatrix, commandClass) {
  const aliases = approvalMatrix?.command_class_aliases || {};
  return aliases[commandClass] || commandClass || "safe";
}

function maxRiskClass(...riskClasses) {
  return riskClasses
    .filter(Boolean)
    .sort((left, right) => (RISK_ORDER[right] ?? -1) - (RISK_ORDER[left] ?? -1))[0] || "low";
}

function stricterApprovalMode(...modes) {
  if (modes.includes("never_auto")) {
    return "never_auto";
  }
  if (modes.includes("manual_required")) {
    return "manual_required";
  }
  return null;
}

function mergeGrantScopes(...scopeLists) {
  const normalized = scopeLists.filter(Array.isArray);
  if (normalized.length === 0) {
    return ["once"];
  }
  return normalized.reduce((current, nextList) => current.filter((item) => nextList.includes(item)));
}

function buildApprovalVerdict(specs, actionContext) {
  const approvalMatrix = specs.approvalMatrix || {};
  const fileClassesSpec = specs.fileClasses || {};
  const normalizedCommandClass = normalizeCommandClass(approvalMatrix, actionContext.commandClass);
  const fileClass = actionContext.fileClass || classifyFilePath(fileClassesSpec, actionContext.targetPath);
  const commandRule = approvalMatrix.command_classes?.[normalizedCommandClass] || approvalMatrix.command_classes?.safe || {};
  const fileRule = fileClass ? approvalMatrix.file_classes?.[fileClass] || {} : {};
  const fileRiskDefault = fileClass ? fileClassesSpec.classes?.[fileClass]?.default_risk_class : null;
  const riskClass = maxRiskClass(actionContext.riskClass, commandRule.risk_class, fileRule.risk_class, fileRiskDefault);
  const approvalRequired = Boolean(commandRule.approval_required || fileRule.approval_required);
  const approvalMode = approvalRequired ? stricterApprovalMode(commandRule.approval_mode, fileRule.approval_mode) : null;
  const allowedGrantScopes = approvalRequired ? mergeGrantScopes(commandRule.allowed_grant_scopes, fileRule.allowed_grant_scopes) : [];
  const eligibility = specs.policySpec?.approval_policy?.notify_only_eligibility || {};
  const notifyOnlyAllowedChangeClasses = approvalMatrix.notify_only?.allowed_change_classes || [];
  const forbiddenCommandClasses = new Set(approvalMatrix.notify_only?.forbidden_command_classes || []);
  const forbiddenFileClasses = new Set(approvalMatrix.notify_only?.forbidden_file_classes || []);
  const notifyOnly =
    !approvalRequired &&
    eligibility.enabled === true &&
    notifyOnlyAllowedChangeClasses.includes(actionContext.changeClass) &&
    actionContext.verificationPassed === true &&
    actionContext.changeIsAuditable === true &&
    !forbiddenCommandClasses.has(normalizedCommandClass) &&
    !forbiddenFileClasses.has(fileClass) &&
    actionContext.touchesAuthOrSecurity !== true &&
    actionContext.touchesSchemaOrPublicApi !== true &&
    actionContext.touchesDataMigration !== true &&
    actionContext.tddExceptionActive !== true;

  return {
    required: approvalRequired,
    resolved: !approvalRequired || actionContext.approvalGranted === true,
    approval_mode: approvalMode,
    allowed_grant_scopes: allowedGrantScopes,
    risk_class: riskClass,
    file_class: fileClass,
    command_class: normalizedCommandClass,
    notify_only: notifyOnly,
  };
}

function isTestGateRequired(node, actionContext) {
  if (actionContext.changeClass === "docs" && actionContext.behaviorChange !== true) {
    return false;
  }
  if (node?.tdd_required) {
    return true;
  }
  if (node?.test_contract) {
    return true;
  }
  return Boolean(actionContext.behaviorChange);
}

function validateRedGate(node, actionContext) {
  const evidence = actionContext.redEvidence;
  if (!evidence) {
    return {
      observed: false,
      red_valid: false,
      reasons: ["red_evidence_missing"],
      pre_write_block: false,
    };
  }
  const contract = node?.test_contract?.red_expectation || { allowed_failure_kinds: [], rejected_failure_kinds: [] };
  const allowedExitCodes = Array.isArray(contract.allowed_exit_codes) && contract.allowed_exit_codes.length > 0 ? contract.allowed_exit_codes : [1];
  const rejectedKinds = new Set(contract.rejected_failure_kinds || []);
  const valid =
    evidence.executed === true &&
    evidence.testFailed === true &&
    evidence.recorded === true &&
    !rejectedKinds.has(evidence.failureKind) &&
    (contract.allowed_failure_kinds || []).includes(evidence.failureKind) &&
    allowedExitCodes.includes(evidence.exitCode);
  return {
    observed: true,
    red_valid: valid,
    reasons: valid ? [] : ["invalid_red_failure_kind"],
    pre_write_block: !valid,
  };
}

function validateGreenGate(actionContext) {
  const evidence = actionContext.greenEvidence;
  if (!evidence) {
    return { observed: false, green_passed: false, reasons: ["green_evidence_missing"] };
  }
  const passed = evidence.executed === true && evidence.passed === true && evidence.newBlockerIntroduced !== true;
  return {
    observed: true,
    green_passed: passed,
    reasons: passed ? [] : ["green_gate_failed"],
  };
}

function validateBehaviorGate(node, actionContext) {
  const contract = node?.test_contract?.behavior_gate;
  const evidence = actionContext.behaviorEvidence;
  if (!contract) {
    return { behavior_pass: true, reasons: [] };
  }
  if (!evidence) {
    return { behavior_pass: false, reasons: ["behavior_evidence_missing"] };
  }
  const traceabilityPass = contract.ac_traceability_required !== true || (evidence.acReferenced === true && evidence.mapped === true);
  const mappedTestsPass = evidence.allMappedTestsPassed === true;
  const passed = traceabilityPass && mappedTestsPass;
  return {
    behavior_pass: passed,
    reasons: passed ? [] : ["behavior_gate_failed"],
  };
}

function validateCoverageGate(node, session, actionContext) {
  const contract = node?.test_contract;
  if (!contract) {
    return { coverage_required: null, coverage_pass: true, reasons: [] };
  }
  const target = COVERAGE_TARGETS[contract.coverage_profile] ?? null;
  if (target === null) {
    return { coverage_required: null, coverage_pass: true, reasons: [] };
  }
  const evidence = actionContext.coverageEvidence;
  if (!evidence) {
    return { coverage_required: target, coverage_pass: false, reasons: ["coverage_evidence_missing"] };
  }
  const envelope = validateCoverageEvidenceEnvelope(evidence);
  if (!envelope.valid) {
    return { coverage_required: target, coverage_pass: false, reasons: [envelope.reason] };
  }
  const thresholdPass = Number(evidence.actual || 0) >= target;
  if (contract.coverage_mode === "patch") {
    const patchPass = evidence.pass === true;
    return {
      coverage_required: target,
      coverage_pass: patchPass,
      reasons: patchPass ? [] : Array.isArray(evidence.reasons) && evidence.reasons.length > 0 ? evidence.reasons : ["patch_coverage_failed"],
    };
  }
  return {
    coverage_required: target,
    coverage_pass: evidence.pass === true && thresholdPass,
    reasons: evidence.pass === true && thresholdPass ? [] : Array.isArray(evidence.reasons) && evidence.reasons.length > 0 ? evidence.reasons : ["coverage_below_threshold"],
  };
}

function buildTestGates(session, node, actionContext) {
  const required = isTestGateRequired(node, actionContext);
  if (!required) {
    return {
      required: false,
      red_valid: true,
      green_passed: true,
      behavior_pass: true,
      coverage_pass: true,
      pre_write_block: false,
      reasons: [],
    };
  }
  const red = validateRedGate(node, actionContext);
  const green = validateGreenGate(actionContext);
  const behavior = validateBehaviorGate(node, actionContext);
  const coverage = validateCoverageGate(node, session, actionContext);
  return {
    required,
    red_valid: red.red_valid,
    green_passed: green.green_passed,
    behavior_pass: behavior.behavior_pass,
    coverage_pass: coverage.coverage_pass,
    pre_write_block: red.pre_write_block,
    coverage_required: coverage.coverage_required,
    reasons: [...red.reasons, ...green.reasons, ...behavior.reasons, ...coverage.reasons],
  };
}

function buildRetryVerdict(node, actionContext) {
  const retryPolicy = node?.retry_policy;
  const retryContext = actionContext.retryContext;
  if (!retryPolicy || !retryContext) {
    return { allowed: false, next_attempt: null, reasons: [] };
  }
  const attemptsUsed = Number(retryContext.attemptsUsed || 0);
  const nextAttempt = attemptsUsed + 1;
  const retryOn = new Set(retryPolicy.retry_on || []);
  const disallowedKinds = new Set(["syntax_error", "deterministic_test_failure", "approval_pending", "invalid_red"]);
  const policyEligible = nextAttempt <= retryPolicy.max_attempts && retryOn.has(retryContext.failureKind) && !disallowedKinds.has(retryContext.failureKind);
  const backoffSatisfied = retryContext.backoffSatisfied !== false;
  const allowed = policyEligible && backoffSatisfied;
  const reasons = [];
  if (!policyEligible) {
    reasons.push("retry_not_allowed");
  }
  if (policyEligible && !backoffSatisfied) {
    reasons.push("retry_backoff_pending");
  }
  return {
    allowed,
    next_attempt: allowed ? nextAttempt : null,
    backoff_mode: retryPolicy.backoff_mode,
    retry_due_at: retryContext.retryDueAt || null,
    retry_wait_ms: retryContext.retryWaitMs ?? null,
    reasons,
  };
}

function buildTimeoutVerdict(node, actionContext) {
  const timeoutPolicy = node?.timeout_policy;
  if (!timeoutPolicy || actionContext.timeoutTriggered !== true) {
    return { triggered: false, on_timeout: null, reasons: [] };
  }
  return {
    triggered: true,
    on_timeout: timeoutPolicy.on_timeout,
    timeout_seconds: timeoutPolicy.timeout_seconds,
    reasons: [],
  };
}

function numericOrNull(value) {
  const normalized = Number(value);
  return Number.isFinite(normalized) ? normalized : null;
}

function withinConfiguredBudget(consumed, max) {
  const maxValue = numericOrNull(max);
  if (maxValue === null) {
    return true;
  }
  return Number(consumed || 0) <= maxValue;
}

function uniqueReasons(reasons) {
  return [...new Set((Array.isArray(reasons) ? reasons : []).filter(Boolean))];
}

function approvalQueueWithinBudget(session) {
  return withinConfiguredBudget(session?.approvals?.pending_count, session?.loop_budget?.max_pending_approvals);
}

function loopBudgetWithinLimit(session) {
  return (
    withinConfiguredBudget(session?.loop_budget?.consumed_nodes, session?.loop_budget?.max_nodes) &&
    withinConfiguredBudget(session?.loop_budget?.consumed_failures, session?.loop_budget?.max_failures)
  );
}

function runtimeFieldsComplete(session, activePhase, activeNode) {
  if (!session || typeof session !== "object") {
    return false;
  }
  if (!session.run_id || !session?.phase?.current || !session?.phase?.status) {
    return false;
  }
  if (!activePhase) {
    return false;
  }
  if (!session?.node || !session.node.owner_persona) {
    return false;
  }
  if (session.node.active_id === undefined || session.node.active_id === null) {
    return false;
  }
  if (session.node.active_id !== "none" && !activeNode) {
    return false;
  }
  return true;
}

function phaseExitGatePassed(session, activePhase, actionContext, verifyPassed) {
  if (!activePhase) {
    return false;
  }
  if (activePhase.status === "completed") {
    return true;
  }
  if (actionContext.phaseExitGateEvidence?.passed === true) {
    return true;
  }
  return verifyPassed === true && session?.phase?.status === "review";
}

function reviewEvidenceReady(reviewPass, actionContext, verifyPassed) {
  if (!reviewPass || verifyPassed !== true) {
    return false;
  }
  if (actionContext.reviewEvidence?.present === false) {
    return false;
  }
  if (actionContext.reviewEvidence?.fresh === false) {
    return false;
  }
  return true;
}

function phaseNodesReadyForCompletion(taskGraph, phaseId, activeNodeId, nodeCompleteReady) {
  const phaseNodes = listPhaseNodes(taskGraph, phaseId);
  if (phaseNodes.length === 0) {
    return false;
  }
  return phaseNodes.every((phaseNode) => phaseNode.status === "completed" || (phaseNode.id === activeNodeId && nodeCompleteReady === true));
}

function phaseHasUnresolvedFailure(taskGraph, phaseId) {
  return listPhaseNodes(taskGraph, phaseId).some((phaseNode) => phaseNode.status === "failed" || reviewFailed(phaseNode.review_state || {}));
}

function hasRemainingIncompletePhase(taskGraph, activePhaseId) {
  const phases = Array.isArray(taskGraph?.phases) ? taskGraph.phases : [];
  const activeIndex = phases.findIndex((phase) => phase.id === activePhaseId);
  if (activeIndex < 0) {
    return false;
  }
  return phases.slice(activeIndex + 1).some((phase) => phase.status !== "completed");
}

function buildCompletionVerdict(session, taskGraph, node, actionContext, testGates) {
  const activePhase = getActivePhase(session, taskGraph);
  const activeNode = getActiveNode(session, taskGraph) || node || null;
  const verifyPassed = actionContext.verifyEvidence?.passed === true;
  const reviewState = activeNode?.review_state || node?.review_state || {};
  const reviewPass = reviewAccepted(reviewState);
  const reviewFail = reviewFailed(reviewState);
  const approvalPending = session?.approvals?.pending === true;
  const queueWithinBudget = approvalQueueWithinBudget(session);
  const nodeReasons = [];

  if (activeNode?.tdd_required === true && testGates.red_valid !== true) {
    nodeReasons.push("valid_red_missing");
  }
  if (testGates.green_passed !== true) {
    nodeReasons.push("green_not_passed");
  }
  if (testGates.behavior_pass !== true) {
    nodeReasons.push("behavior_gate_failed");
  }
  if (testGates.coverage_pass !== true) {
    nodeReasons.push("coverage_gate_failed");
  }
  if (verifyPassed !== true) {
    nodeReasons.push("verify_not_passed");
  }
  if (reviewFail) {
    nodeReasons.push("review_failed");
  } else if (!reviewPass) {
    nodeReasons.push("review_not_passed");
  }
  if (approvalPending) {
    nodeReasons.push("approval_pending");
  }
  if (!queueWithinBudget) {
    nodeReasons.push("approval_queue_exceeded");
  }

  const nodeCompleteReady = nodeReasons.length === 0;
  const phaseExitPassed = phaseExitGatePassed(session, activePhase, actionContext, verifyPassed);
  const phaseReasons = [];

  if (!activePhase || !phaseNodesReadyForCompletion(taskGraph, activePhase.id, activeNode?.id, nodeCompleteReady)) {
    phaseReasons.push("phase_nodes_incomplete");
  }
  if (activePhase && phaseHasUnresolvedFailure(taskGraph, activePhase.id)) {
    phaseReasons.push("phase_has_failed_or_rework_node");
  }
  if (!phaseExitPassed) {
    phaseReasons.push("phase_exit_gate_not_passed");
  }
  if (approvalPending) {
    phaseReasons.push("approval_pending");
  }
  if (!queueWithinBudget) {
    phaseReasons.push("approval_queue_exceeded");
  }
  if (session?.recovery?.restore_pending === true) {
    phaseReasons.push("restore_pending");
  }

  const phaseCompleteReady = activePhase !== null && phaseReasons.length === 0;
  const stopReasons = [];

  if (approvalPending) {
    stopReasons.push("approval_pending");
  }
  if (verifyPassed !== true) {
    stopReasons.push("active_node_verification_incomplete");
  }
  if (!phaseExitPassed) {
    stopReasons.push("phase_exit_gate_not_passed");
  }
  if (!reviewEvidenceReady(reviewPass, actionContext, verifyPassed)) {
    stopReasons.push("review_evidence_not_ready");
  }
  if (!runtimeFieldsComplete(session, activePhase, activeNode)) {
    stopReasons.push("runtime_fields_incomplete");
  }
  if (!queueWithinBudget) {
    stopReasons.push("approval_queue_exceeded");
  }
  if (!loopBudgetWithinLimit(session)) {
    stopReasons.push("loop_budget_exhausted");
  }
  if (!phaseCompleteReady) {
    stopReasons.push("phase_completion_incomplete");
  }
  if (activePhase && hasRemainingIncompletePhase(taskGraph, activePhase.id)) {
    stopReasons.push("remaining_phase_work");
  }

  return {
    node_complete_ready: nodeCompleteReady,
    phase_complete_ready: phaseCompleteReady,
    stop_gate_ready: stopReasons.length === 0,
    reasons: uniqueReasons([...nodeReasons, ...phaseReasons, ...stopReasons]),
  };
}

function evaluatePolicy(input) {
  const session = input.session || {};
  const taskGraph = input.taskGraph || {};
  const actionContext = input.actionContext || {};
  const specs = input.specs || {};
  const node = getActiveNode(session, taskGraph) || (Array.isArray(taskGraph.nodes) ? taskGraph.nodes[0] : null);
  const approval = buildApprovalVerdict(specs, actionContext);
  const testGates = buildTestGates(session, node, actionContext);
  const retry = buildRetryVerdict(node, actionContext);
  const timeout = buildTimeoutVerdict(node, actionContext);
  const completion = buildCompletionVerdict(session, taskGraph, node, actionContext, testGates);

  let routeEffect = "allow";
  let primaryReason = approval.notify_only ? "notify_only" : null;

  if (timeout.triggered && timeout.on_timeout === "require_human") {
    routeEffect = "require_human";
    primaryReason = "node_timed_out";
  } else if (approval.required && approval.resolved !== true) {
    routeEffect = "require_human";
    primaryReason = "approval_required";
  } else if (actionContext.redEvidence && testGates.red_valid !== true) {
    routeEffect = "block";
    primaryReason = "invalid_red";
  } else if (reviewFailed(node?.review_state || {})) {
    routeEffect = "hold";
    primaryReason = "review_failed";
  }

  return {
    route_effect: routeEffect,
    primary_reason: primaryReason,
    quality_level: routeEffect === "allow" ? "PASS" : routeEffect === "hold" ? "CONCERNS" : routeEffect === "block" ? "REWORK" : "FAIL",
    approval,
    test_gates: testGates,
    retry,
    timeout,
    completion,
    target_ref: actionContext.targetPath || node?.target || node?.id || null,
  };
}

module.exports = {
  classifyFilePath,
  evaluatePolicy,
  normalizeCommandClass,
};
