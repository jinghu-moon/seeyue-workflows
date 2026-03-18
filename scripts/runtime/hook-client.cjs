"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { execSync } = require("node:child_process");

const {
  asObject,
  compilePattern,
  countLedgerNodes,
  isInsideTarget,
  loadDebugState,
  loadPolicy,
  loadWorkflowState,
  parseJsonSafe,
  readStdin,
  resolveCwd,
  resolveContent,
  resolveFilePath,
  resolveInput,
  scanSecrets,
  warn,
} = require("../hooks/sy-hook-lib.cjs");
const { appendEvent } = require("./journal.cjs");
const { classifyFilePath, evaluatePolicy } = require("./policy.cjs");
const { buildResumeFrontier, ensurePreDestructiveCheckpoint } = require("./checkpoints.cjs");
const { buildShortApprovalRequest, buildShortRestoreRequest } = require("./human-output.cjs");
const { appendOutputLogs } = require("./output-log.cjs");
const { validateOutputEntries } = require("./validate-output.cjs");
const { validateWorkflowSpecs } = require("./spec-validator.cjs");
const { loadWorkflowSpecs } = require("./workflow-specs.cjs");
const { loadYamlFile } = require("./yaml-loader.cjs");
const {
  getActiveNode,
  getActivePhase,
  getRecommendedNext,
  hasCompleteRuntime,
  isApprovalPending,
  isRestorePending,
  loadRuntimeSnapshot,
  projectWorkflowCompatState,
} = require("./runtime-snapshot.cjs");

const TEST_PATTERNS = [/[._-]test[._/-]/, /[._-]spec[._/-]/, /__tests?__/, /\.test\.[a-z]+$/, /\.spec\.[a-z]+$/];
const V4_RED_READY_STATES = new Set(["red_verified", "green_pending", "green_verified", "refactor_pending", "verified"]);
const STOP_LOCK_PATH = path.join(os.tmpdir(), ".sy_stop_hook_active");

let cachedSpecs = null;
let cachedHookContract = null;
let cachedHookContractRoot = null;
let cachedHookGate = null;
let cachedHookGateRoot = null;

const HOOK_CONTRACT_SCHEMA_KIND = "hook_contract_schema";
const HOOK_CONTRACT_SCHEMA_VERSION = 1;
const HOOK_CLIENT_FREEZE_GATE = "P3-N1";
const HOOK_CLIENT_REQUIRED_SPECS = [
  "workflow/hooks.spec.yaml",
  "workflow/hook-contract.schema.yaml",
];

const MEDIUM_CONFIDENCE = [
  { name: "long alphanumeric assignment", regex: /(api[_-]?key|secret|token|password)\s*[:=]\s*["'`]?([A-Za-z0-9+/]{32,})["'`]?/i },
  { name: "bearer token in string", regex: /["']Bearer\s+[A-Za-z0-9._\-]{20,}["']/i },
];


const PHASE_CLASSIFIERS = [
  { phase: "build", patterns: [/\bcargo\s+build\b/i, /\bnpm\s+run\s+build\b/i, /\bvite\s+build\b/i, /\bgo\s+build\b/i] },
  { phase: "typecheck", patterns: [/\bcargo\s+check\b/i, /\btsc\s+--no-emit\b/i, /\bvue-tsc\b/i, /\bgo\s+vet\b/i, /\bmypy\b/i] },
  { phase: "lint", patterns: [/\bcargo\s+clippy\b/i, /\beslint\b/i, /\bruff\s+check\b/i, /\bflake8\b/i, /\bgolangci-lint\b/i] },
  { phase: "test", patterns: [/\bcargo\s+test\b/i, /\bnpm\s+test\b/i, /\bvitest\s+run\b/i, /\bpytest\b/i, /\bgo\s+test\b/i, /\bjest\b/i, /\bmocha\b/i] },
  { phase: "security", patterns: [/\bcargo\s+audit\b/i, /\bnpm\s+audit\b/i, /\btrufflehog\b/i, /\bgitleaks\b/i, /\bsemgrep\b/i] },
];

const LOOP_BUDGET_EXECUTION_PATTERNS = [
  /\bcargo\s+(test|build|check|clippy|run)\b/i,
  /\bnpm\s+(test|run|build)\b/i,
  /\bpnpm\s+(test|run|build)\b/i,
  /\bbun\s+(test|run|build)\b/i,
  /\bdeno\s+(test|run|build)\b/i,
  /\bpytest\b/i,
  /\bgo\s+(test|build|run)\b/i,
  /\bnpx\s+(jest|vitest|mocha|tsc)\b/i,
  /\bvitest\s+run\b/i,
  /\beslint\b/i,
  /\bruff\s+check\b/i,
  /\bmypy\b/i,
];

const LEGAL_LEGACY_PHASES = new Set([
  "exploring",
  "benchmarking",
  "free-ideation",
  "designing",
  "plan",
  "execute",
  "review",
  "done",
  "explore",
  "benchmark",
  "design",
  "brainstorm",
  "ideation",
  "",
]);

const LEGAL_V4_PHASE_STATUSES = new Set([
  "pending",
  "in_progress",
  "blocked",
  "review",
  "completed",
  "",
]);

const REGRESSION_MAP = {
  review: new Set(["plan", "execute"]),
  done: new Set(["plan", "execute", "review"]),
};

const RUN_ID_PATTERN = /^wf-\d{8}-\d{3}$/;

const PASS_SIGNALS = [
  /\b0\s+(errors?|failed|failures?)\b/i,
  /\bfinished\b.*\b0\s+errors?\b/i,
  /\ball\s+tests?\s+pass(ed)?\b/i,
  /\btest\s+result\b.*\bok\b/i,
  /\bBUILD\s+SUCCESSFUL\b/i,
  /\bbuilt\s+in\b/i,
  /\bno\s+(issues?|errors?|warnings?)\s+found\b/i,
];

const FAIL_SIGNALS = [
  /\b\d+\s+(errors?|failures?|failed)\b/i,
  /\bBUILD\s+FAILED\b/i,
  /\berror\[E\d+\]/i,
  /\bFAILED\b/,
  /\btest\s+result\b.*\bFAILED\b/i,
];

const FAILURE_KIND_PATTERNS = [
  { kind: "syntax_error", patterns: [/syntaxerror/i, /unexpected token/i, /parse error/i] },
  { kind: "import_error", patterns: [/module not found/i, /cannot find module/i, /importerror/i, /no module named/i] },
  { kind: "permission_error", patterns: [/permission denied/i, /eacces/i, /eperm/i] },
  { kind: "connection_error", patterns: [/econnrefused/i, /enotfound/i, /timed out/i, /timeout/i, /connection refused/i] },
  { kind: "fixture_initialization_error", patterns: [/fixture/i, /beforeall/i, /before each/i, /setup failed/i] },
  { kind: "contract_mismatch", patterns: [/contract/i, /schema mismatch/i] },
  { kind: "expected_validation_failure", patterns: [/validation failed/i, /expected validation/i] },
  { kind: "behavior_result_mismatch", patterns: [/behavior mismatch/i, /unexpected behavior/i] },
  { kind: "assertion_failure", patterns: [/assertion/i, /expected .* to/i, /\bshould\b/i, /\bexpect\b/i, /\bassert\b/i] },
];

function normalizeInstructions(value) {
  if (!value) {
    return [];
  }
  if (Array.isArray(value)) {
    return value.filter(Boolean).map((item) => String(item));
  }
  return [String(value)];
}

function normalizeStringList(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  const items = value.map((entry) => String(entry || "").trim()).filter(Boolean);
  return Array.from(new Set(items));
}

function normalizeBoolean(value) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (normalized === "true") {
      return true;
    }
    if (normalized === "false") {
      return false;
    }
  }
  return false;
}

function normalizeTddException(value) {
  const record = asObject(value);
  if (Object.keys(record).length === 0) {
    return null;
  }
  return {
    reason: String(record.reason || "").trim(),
    alternative_verification: String(record.alternative_verification || "").trim(),
    user_approved: normalizeBoolean(record.user_approved),
  };
}

function normalizeScopeTarget(value) {
  const trimmed = String(value || "").trim().replace(/`/g, "").replace(/\\/g, "/");
  if (!trimmed) {
    return null;
  }
  if (!/^[A-Za-z0-9_./-]+$/.test(trimmed)) {
    return null;
  }
  return trimmed;
}

function formatTaggedLines(tag, label, reason, extraLines) {
  const lines = [`[${tag}] ${label}: ${reason}`];
  for (const line of normalizeInstructions(extraLines)) {
    lines.push(`[${tag}] ${line}`);
  }
  return lines;
}

function normalizeOutputTemplates(outputTemplates) {
  const list = Array.isArray(outputTemplates) ? outputTemplates : [];
  return list.filter((entry) => entry && typeof entry === "object");
}

function persistOutputTemplates(cwd, outputTemplates) {
  const list = normalizeOutputTemplates(outputTemplates);
  if (list.length === 0) {
    return;
  }
  const specs = loadWorkflowSpecs(resolveProjectRoot(cwd));
  const templates = specs && specs.outputTemplates && specs.outputTemplates.templates
    ? specs.outputTemplates.templates
    : {};
  const validation = validateOutputEntries(list, templates);
  if (!validation.ok) {
    const issues = validation.issues.map((issue) => issue.code + ':' + issue.message).join('; ');
    throw new Error('OUTPUT_TEMPLATE_VALIDATION_FAIL ' + issues);
  }
  const logEntries = list.map((entry) => ({
    ...entry,
    recorded_at: new Date().toISOString(),
  }));
  appendOutputLogs(cwd, logEntries);
}

function buildOutputEnvelope(result) {
  const envelope = {
    verdict: String(result.verdict || "allow"),
    reason: String(result.reason || "allow"),
    instructions: normalizeInstructions(result.instructions),
    ...(result.approvalRequest ? { approval_request: result.approvalRequest } : {}),
    ...(result.outputTemplates ? { output_templates: result.outputTemplates } : {}),
    ...(result.inputMutation ? { input_mutation: result.inputMutation } : {}),
    ...(result.journalEvents ? { journal_events: result.journalEvents } : {}),
    ...(result.metadata ? { metadata: result.metadata } : {}),
  };
  if (result.extraOutput && typeof result.extraOutput === "object") {
    for (const [key, value] of Object.entries(result.extraOutput)) {
      if (!(key in envelope)) {
        envelope[key] = value;
      }
    }
  }
  // Merge interaction envelope fields (P2-N2)
  const interactionEnvelope = buildInteractionEnvelope(result);
  for (const [key, value] of Object.entries(interactionEnvelope)) {
    if (!(key in envelope)) {
      envelope[key] = value;
    }
  }
  return envelope;
}

/**
 * buildInteractionEnvelope — P2-N2
 *
 * Enriches the output envelope with interaction-awareness fields.
 * Does NOT create interaction store entries — runtime kernel decides that.
 *
 * Fields added:
 *   interaction_required  {boolean}         true when an interaction is needed
 *   interaction_kind      {string|null}      'approval_request'|'restore_request'|'question_request'|'input_request'|null
 *   blocking_kind         {string|null}      e.g. 'hard_gate'|'advisory'|null
 *   reason_code           {string|null}      machine-readable reason tag
 *   risk_level            {string|null}      'low'|'medium'|'high'|'critical'|null
 *   scope                 {string|null}      file path or command scope context
 */
function buildInteractionEnvelope(result) {
  const verdict = String(result.verdict || "allow");
  const isBlocking = verdict === "block" || verdict === "block_with_approval_request";
  const hasApproval = verdict === "block_with_approval_request" || !!result.approvalRequest;
  const isRestorePendingFlag = result.metadata && result.metadata.restore_pending;

  // Determine interaction_kind
  let interactionKind = null;
  if (hasApproval) {
    interactionKind = "approval_request";
  } else if (isRestorePendingFlag) {
    interactionKind = "restore_request";
  } else if (result.metadata && result.metadata.question_pending) {
    interactionKind = "question_request";
  } else if (result.metadata && result.metadata.input_pending) {
    interactionKind = "input_request";
  }

  // Determine blocking_kind
  let blockingKind = null;
  if (isBlocking) {
    blockingKind = "hard_gate";
  } else if (verdict === "force_continue") {
    blockingKind = "advisory";
  }

  // Extract reason_code from metadata or approval request
  let reasonCode = null;
  if (result.metadata && result.metadata.reason_code) {
    reasonCode = String(result.metadata.reason_code);
  } else if (result.approvalRequest && result.approvalRequest.reason_code) {
    reasonCode = String(result.approvalRequest.reason_code);
  } else if (isBlocking) {
    // Derive from reason string if present
    const reason = String(result.reason || "");
    const codeMatch = reason.match(/\[([A-Z_]+)\]/);
    reasonCode = codeMatch ? codeMatch[1] : null;
  }

  // Risk level
  let riskLevel = null;
  if (result.metadata && result.metadata.risk_level) {
    riskLevel = String(result.metadata.risk_level);
  } else if (result.approvalRequest && result.approvalRequest.risk_level) {
    riskLevel = String(result.approvalRequest.risk_level);
  } else if (blockingKind === "hard_gate") {
    riskLevel = "high";
  }

  // Scope
  let scope = null;
  if (result.metadata && result.metadata.scope) {
    scope = String(result.metadata.scope);
  } else if (result.metadata && result.metadata.file_path) {
    scope = String(result.metadata.file_path);
  } else if (result.metadata && result.metadata.command) {
    scope = String(result.metadata.command).slice(0, 120);
  }

  return {
    interaction_required: interactionKind !== null,
    interaction_kind: interactionKind,
    blocking_kind: blockingKind,
    reason_code: reasonCode,
    risk_level: riskLevel,
    scope,
  };
}

function persistOutputTemplatesForTest(rootDir, outputTemplates) {
  persistOutputTemplates(rootDir, outputTemplates);
}
function emitResult(result) {
  const exitCode = Number.isInteger(result.exitCode)
    ? result.exitCode
    : result.verdict === "block" || result.verdict === "block_with_approval_request"
      ? 2
      : 0;

  if (Array.isArray(result.outputTemplates) && result.outputTemplates.length > 0) {
    const targetRoot = resolveProjectRoot(result.cwd);
    persistOutputTemplates(targetRoot, result.outputTemplates);
  }
  process.stdout.write(JSON.stringify(buildOutputEnvelope(result)));
  if (Array.isArray(result.stderrLines) && result.stderrLines.length > 0) {
    process.stderr.write(result.stderrLines.join("\n") + "\n");
  }
  process.exit(exitCode);
}

function buildAllowResult(reason, instructions, metadata, extraOutput) {
  return {
    verdict: "allow",
    reason: reason || "allow",
    instructions: normalizeInstructions(instructions),
    metadata: metadata || null,
    extraOutput: extraOutput || null,
    exitCode: 0,
  };
}

function buildBlockResult(tag, reason, instructions, approvalRequest) {
  return {
    verdict: approvalRequest ? "block_with_approval_request" : "block",
    reason,
    instructions: normalizeInstructions(instructions),
    approvalRequest: approvalRequest || null,
    stderrLines: formatTaggedLines(tag, "BLOCKED", reason, instructions),
    exitCode: 2,
  };
}

function buildForceContinueResult(tag, reason, instructions, metadata) {
  return {
    verdict: "force_continue",
    reason,
    instructions: normalizeInstructions(instructions),
    metadata: metadata || null,
    stderrLines: formatTaggedLines(tag, "FORCE_CONTINUE", reason, instructions),
    exitCode: 0,
  };
}

function looksLikeExecutionCommand(command) {
  return LOOP_BUDGET_EXECUTION_PATTERNS.some((pattern) => pattern.test(command));
}

function formatFirstRecommendedNext(items) {
  const nextItems = Array.isArray(items) ? items : [];
  if (nextItems.length === 0) {
    return "(none)";
  }
  const first = nextItems[0] || {};
  return `${String(first.type || "unknown")}:${String(first.target || "unknown")}`;
}

function countVerifiedNodes(auditPath) {
  try {
    if (!fs.existsSync(auditPath)) {
      return 0;
    }
    const lines = fs.readFileSync(auditPath, "utf8").split("\n").filter(Boolean);
    const nodes = new Set();
    for (const line of lines) {
      try {
        const entry = JSON.parse(line);
        if (entry.event === "VERIFY_PASS" && entry.node) {
          nodes.add(String(entry.node));
        }
      } catch {
        // skip malformed lines
      }
    }
    return nodes.size;
  } catch {
    return 0;
  }
}

function countConsecutiveFailures(auditPath) {
  try {
    if (!fs.existsSync(auditPath)) {
      return 0;
    }
    const lines = fs.readFileSync(auditPath, "utf8").split("\n").filter(Boolean).reverse();
    let count = 0;
    for (const line of lines) {
      try {
        const entry = JSON.parse(line);
        if (entry.event === "VERIFY_FAIL") {
          count += 1;
          continue;
        }
        if (entry.event === "VERIFY_PASS") {
          break;
        }
      } catch {
        // skip malformed lines
      }
    }
    return count;
  } catch {
    return 0;
  }
}

function isSessionFile(filePath) {
  const normalized = String(filePath || "").replace(/\\/g, "/").toLowerCase();
  return normalized.endsWith(".ai/workflow/session.yaml") || normalized.endsWith(".ai/workflow/session.md");
}

function extractLegacyPhase(content) {
  const match = String(content || "").match(/^[\s\-]*current_phase\s*:\s*(\S+)/im);
  return match ? match[1].trim().toLowerCase().replace(/['"]/g, "") : null;
}

function extractRunId(content) {
  const match = String(content || "").match(/^[\s\-]*run_id\s*:\s*(\S+)/im);
  return match ? match[1].trim().replace(/['"` ]/g, "") : null;
}

function extractNestedField(content, parentKey, fieldKey) {
  const lines = String(content || "").split(/\r?\n/);
  let inside = false;
  let parentIndent = -1;

  for (const line of lines) {
    const trimmed = line.trim();
    if (!inside) {
      if (trimmed === `${parentKey}:`) {
        inside = true;
        parentIndent = line.search(/\S|$/);
      }
      continue;
    }

    if (trimmed.length === 0) {
      continue;
    }

    const indent = line.search(/\S|$/);
    if (indent <= parentIndent) {
      inside = false;
      if (trimmed === `${parentKey}:`) {
        inside = true;
        parentIndent = indent;
      }
      continue;
    }

    if (trimmed.startsWith(`${fieldKey}:`)) {
      return trimmed.slice(fieldKey.length + 1).trim().toLowerCase().replace(/['"]/g, "");
    }
  }

  return null;
}

function isProductionCode(filePath) {
  return Boolean(filePath) && !TEST_PATTERNS.some((re) => re.test(filePath));
}

function getProjectRoot() {
  return path.resolve(__dirname, "..", "..");
}

function resolveProjectRoot(cwd) {
  try {
    return path.resolve(cwd || getProjectRoot());
  } catch {
    return getProjectRoot();
  }
}

function getWorkflowSpecs() {
  if (!cachedSpecs) {
    try {
      cachedSpecs = loadWorkflowSpecs(getProjectRoot());
    } catch {
      cachedSpecs = {};
    }
  }
  return cachedSpecs;
}

function loadHookContractSpec(rootDir) {
  if (cachedHookContract && cachedHookContractRoot === rootDir) {
    return cachedHookContract;
  }
  const specPath = path.join(rootDir, "workflow", "hook-contract.schema.yaml");
  const spec = loadYamlFile(specPath);
  cachedHookContract = spec;
  cachedHookContractRoot = rootDir;
  return spec;
}

function validateHookContractVersion(rootDir) {
  let spec;
  try {
    spec = loadHookContractSpec(rootDir);
  } catch (error) {
    return {
      ok: false,
      code: "HOOK_CONTRACT_MISSING",
      message: String(error?.message || error || "hook contract missing"),
    };
  }
  if (!spec || typeof spec !== "object") {
    return {
      ok: false,
      code: "HOOK_CONTRACT_INVALID",
      message: "hook contract spec is not an object",
    };
  }
  const schemaKind = String(spec.schema_kind || "").trim();
  if (schemaKind !== HOOK_CONTRACT_SCHEMA_KIND) {
    return {
      ok: false,
      code: "HOOK_CONTRACT_SCHEMA_KIND_MISMATCH",
      message: `expected schema_kind=${HOOK_CONTRACT_SCHEMA_KIND}`,
      actual: schemaKind,
    };
  }
  const schemaVersion = Number(spec.schema_version);
  if (!Number.isFinite(schemaVersion)) {
    return {
      ok: false,
      code: "HOOK_CONTRACT_SCHEMA_VERSION_INVALID",
      message: "schema_version must be a number",
    };
  }
  if (schemaVersion !== HOOK_CONTRACT_SCHEMA_VERSION) {
    return {
      ok: false,
      code: "HOOK_CONTRACT_SCHEMA_VERSION_MISMATCH",
      message: `expected schema_version=${HOOK_CONTRACT_SCHEMA_VERSION}`,
      actual: schemaVersion,
    };
  }
  return { ok: true };
}

function validateHookClientFreezeGate(rootDir) {
  if (cachedHookGate && cachedHookGateRoot === rootDir) {
    return cachedHookGate;
  }
  const result = validateWorkflowSpecs({
    rootDir,
    specPaths: HOOK_CLIENT_REQUIRED_SPECS,
    validateScope: "envelope",
    freezeGate: HOOK_CLIENT_FREEZE_GATE,
  });
  const issues = Array.isArray(result.issues) ? result.issues : [];
  const ok = issues.length === 0;
  cachedHookGate = { ok, issues };
  cachedHookGateRoot = rootDir;
  return cachedHookGate;
}

function guardHookClientContracts(payload) {
  const cwd = resolveCwd(payload);
  const rootDir = resolveProjectRoot(cwd);
  const contractCheck = validateHookContractVersion(rootDir);
  if (!contractCheck.ok) {
    const details = [
      `错误码: ${contractCheck.code}`,
      contractCheck.message ? `原因: ${contractCheck.message}` : null,
      `期望 schema_version: ${HOOK_CONTRACT_SCHEMA_VERSION}`,
      contractCheck.actual !== undefined ? `实际 schema_version: ${contractCheck.actual}` : null,
      `来源: workflow/hook-contract.schema.yaml`,
    ].filter(Boolean);
    return buildBlockResult(
      "sy-hook-contract",
      "Hook 合约校验失败，Hook Client 已拒绝启动",
      details,
    );
  }
  const gateCheck = validateHookClientFreezeGate(rootDir);
  if (!gateCheck.ok) {
    const issues = gateCheck.issues.map((issue) => `${issue.code} ${issue.specPath} ${issue.message}`);
    return buildBlockResult(
      "sy-hook-freeze-gate",
      "Hook Client 依赖规范未冻结，禁止进入执行阶段",
      [
        `freeze_gate: ${HOOK_CLIENT_FREEZE_GATE}`,
        ...issues,
      ],
    );
  }
  return null;
}

function normalizeTargetPath(cwd, filePath) {
  if (!filePath) {
    return "";
  }
  const absolute = path.isAbsolute(filePath) ? filePath : path.resolve(cwd, filePath);
  const relative = path.relative(cwd, absolute);
  return String(relative || filePath).replace(/\\/g, "/");
}

function findLatestRedEvidence(snapshot, nodeId) {
  const journal = Array.isArray(snapshot?.journal) ? snapshot.journal : [];
  for (let index = journal.length - 1; index >= 0; index -= 1) {
    const event = journal[index];
    if (!event || event.event !== "red_recorded") {
      continue;
    }
    const eventNodeId = String(event.node_id || event.nodeId || "").trim();
    if (nodeId && eventNodeId && eventNodeId !== nodeId) {
      continue;
    }
    const payload = asObject(event.payload);
    return {
      executed: payload.executed === true,
      testFailed: payload.testFailed === true,
      failureKind: String(payload.failureKind || payload.failure_kind || "").trim(),
      exitCode: Number(payload.exitCode ?? payload.exit_code ?? 0),
      recorded: payload.recorded === true,
    };
  }
  return null;
}

function deriveChangeClass(fileClass) {
  if (fileClass === "docs_file") {
    return "docs";
  }
  if (fileClass === "generated_file") {
    return "scaffold";
  }
  if (fileClass === "test_file") {
    return "utility";
  }
  return "feature";
}

function checkPersonaWritePermission(snapshot, specs) {
  const persona = snapshot?.session?.node?.owner_persona;
  if (!persona) {
    return null;
  }
  const personaBindings = specs?.personaBindings?.personas || {};
  const personaDef = personaBindings[persona];
  if (!personaDef) {
    return null; // unknown persona — do not block
  }
  if (personaDef.may_write_files === false) {
    return { blocked: true, reason: `Persona ${persona} may not write files` };
  }
  return null;
}

function checkPersonaCommandPermission(snapshot, commandClass, specs) {
  const persona = snapshot?.session?.node?.owner_persona;
  if (!persona) {
    return null;
  }
  const personaBindings = specs?.personaBindings?.personas || {};
  const personaDef = personaBindings[persona];
  if (!personaDef) {
    return null;
  }
  if (personaDef.may_run_commands === false) {
    return { blocked: true, reason: `Persona ${persona} may not run commands` };
  }
  const reviewerPersonas = new Set(["spec_reviewer", "quality_reviewer"]);
  const destructiveClasses = new Set(["destructive", "privileged", "git_mutating"]);
  if (reviewerPersonas.has(persona) && destructiveClasses.has(commandClass)) {
    return { blocked: true, reason: `Persona ${persona} may not run ${commandClass} commands` };
  }
  return null;
}

function hasApprovedWriteGrant(snapshot, targetPath, fileClass) {
  const grants = Array.isArray(snapshot?.session?.approvals?.grants) ? snapshot.session.approvals.grants : [];
  return grants.some((grant) => {
    const decision = String(grant?.decision || "").toLowerCase();
    if (decision !== "approved") {
      return false;
    }
    const action = String(grant?.action || "").toLowerCase();
    if (action && !["write", "write_file", "edit", "edit_file"].includes(action)) {
      return false;
    }
    const targetRef = String(grant?.target_ref || "").trim();
    return [targetPath, fileClass, "*"].includes(targetRef);
  });
}

function buildPolicyContext(state, cwd, filePath) {
  const specs = getWorkflowSpecs();
  const targetPath = normalizeTargetPath(cwd, filePath);
  const fileClass = classifyFilePath(specs.fileClasses, targetPath);
  const snapshot = state.runtimeReady ? state.snapshot : null;
  const redEvidence = snapshot ? findLatestRedEvidence(snapshot, state.activeNodeId) : null;
  const approvalGranted = snapshot ? hasApprovedWriteGrant(snapshot, targetPath, fileClass) : false;
  const tddException = snapshot ? normalizeTddException(snapshot.activeNode?.tdd_exception) : null;
  const tddExceptionActive = Boolean(tddException);

  return {
    specs,
    targetPath,
    fileClass,
    snapshot,
    redEvidence,
    tddException,
    tddExceptionActive,
    actionContext: {
      targetPath,
      fileClass,
      commandClass: "safe",
      changeClass: deriveChangeClass(fileClass),
      behaviorChange: isProductionCode(targetPath),
      verificationPassed: false,
      changeIsAuditable: true,
      approvalGranted,
      tddExceptionActive,
      touchesAuthOrSecurity: fileClass === "security_boundary",
      touchesSchemaOrPublicApi: ["critical_policy_file", "security_boundary"].includes(fileClass),
      touchesDataMigration: false,
      redEvidence,
    },
  };
}

function shouldCreatePreDestructiveCheckpoint(state, cwd, filePath) {
  if (!state.runtimeReady || !state.snapshot?.activeNode || !filePath) {
    return false;
  }
  const normalizedPath = normalizeTargetPath(cwd, filePath);
  if (!normalizedPath || normalizedPath.startsWith(".ai/")) {
    return false;
  }
  if (/\.(md|jsonl|json|yaml|yml|txt|log)$/i.test(normalizedPath)) {
    return false;
  }
  const absolutePath = path.isAbsolute(filePath) ? filePath : path.resolve(cwd, filePath);
  return fs.existsSync(absolutePath);
}

function buildApprovalLines(targetPath, verdict) {
  const approval = verdict.approval || {};
  return buildShortApprovalRequest({
    actionLabel: "写入文件",
    targetRef: `${targetPath} (${approval.file_class || "unknown"})`,
    riskClass: approval.risk_class || "unknown",
    approvalMode: approval.approval_mode || "manual_required",
    grantScopes: Array.isArray(approval.allowed_grant_scopes) && approval.allowed_grant_scopes.length > 0
      ? approval.allowed_grant_scopes
      : ["once"],
  });
}

function blockForInvalidRed(filePath, nodeId, redEvidence) {
  return buildBlockResult(
    "sy-pretool-write/gate2-invalid-red",
    "RED 测试证据无效，禁止写入生产代码",
    [
      filePath ? `文件：${filePath}` : "",
      nodeId ? `节点：${nodeId}` : "",
      `失败类型：${redEvidence?.failureKind || "unknown"}`,
      `退出码：${redEvidence?.exitCode ?? "unknown"}`,
      "要求：先修复环境/夹具问题，重新拿到合法的失败测试证据，再继续写实现。",
    ].filter(Boolean),
  );
}

function blockForMissingRed(filePath, nodeId, tddState) {
  return buildBlockResult(
    "sy-pretool-write/gate2-tdd",
    "TDD 红灯未完成，禁止写入生产代码",
    [
      filePath ? `文件：${filePath}` : "",
      nodeId ? `节点：${nodeId}` : "",
      `当前状态：${tddState || "unknown"}`,
      "要求：先补失败测试并记录 RED 证据，再继续写实现代码。",
    ].filter(Boolean),
  );
}

function handlePretoolWrite(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const cfg = asObject(policy.pretoolWrite);

  const bypass = String(cfg.bypassEnv || "SY_BYPASS_PRETOOL_WRITE");
  if (process.env[bypass] === "1") {
    return buildAllowResult("bypass_pretool_write");
  }

  const toolName = String(payload.tool_name || payload.tool || "").toLowerCase();
  if (!/(write|edit)/i.test(toolName)) {
    return buildAllowResult("non_write_tool");
  }

  const filePath = resolveFilePath(payload);
  const content = resolveContent(payload);

  const protectedFiles = Array.isArray(cfg.protectedFiles) ? cfg.protectedFiles : [];
  for (const item of protectedFiles) {
    const protectedFile = asObject(item);
    const regex = compilePattern(protectedFile.pattern, "i");
    if (regex && regex.test(filePath)) {
      return buildBlockResult(
        "sy-pretool-write/gate1-protected",
        `protected file — do not write directly: ${filePath} (${String(protectedFile.label || "protected")})`,
        [
          "Use the appropriate CLI command or update via project tooling.",
          "See: sy-constraints/execution",
        ],
      );
    }
  }

  const state = loadWorkflowState(cwd, policy);
  const policyContext = filePath ? buildPolicyContext(state, cwd, filePath) : null;

  // Persona write permission check (isolation_enforcement)
  if (state.runtimeReady && state.snapshot) {
    const personaWriteBlock = checkPersonaWritePermission(state.snapshot, getWorkflowSpecs());
    if (personaWriteBlock?.blocked) {
      return buildBlockResult(
        "sy-pretool-write/gate1-persona",
        personaWriteBlock.reason,
        ["当前 persona 无文件写入权限，请切换到 author persona 后再继续。"],
      );
    }
  }

  const targetPath = policyContext?.targetPath || filePath;
  const fileClass = policyContext?.fileClass || null;
  const isRuntimeStateFile = Boolean(targetPath && String(targetPath).startsWith(".ai/"));
  const isEvidenceFile = filePath ? /\.(md|jsonl|json|yaml|yml|txt|log)$/i.test(filePath) : false;

  if (filePath && !fileClass && !isRuntimeStateFile) {
    return buildBlockResult(
      "sy-pretool-write/gate1-file-class",
      "无法识别文件分类，拒绝写入",
      [
        `file: ${filePath}`,
        "请先补充 workflow/file-classes.yaml 的匹配规则，再继续写入。",
      ],
    );
  }

  if (state.runtimeReady && approvalQueueExceeded(state.snapshot?.session || {})) {
    return buildBlockResult(
      "sy-pretool-write/gate1-approval-queue",
      "审批队列超过上限，拒绝写入",
      [
        `pending_count: ${state.snapshot?.session?.approvals?.pending_count ?? "unknown"}`,
        `max_pending_approvals: ${state.snapshot?.session?.loop_budget?.max_pending_approvals ?? "unknown"}`,
        "请先处理待审批项或调整预算上限。",
      ],
    );
  }

  const shouldValidateRuntime =
    Boolean(
      state.runtimeReady
      && state.snapshot
      && filePath
      && isProductionCode(filePath)
      && !isRuntimeStateFile
      && fileClass
      && !["docs_file", "test_file"].includes(fileClass),
    );

  if (shouldValidateRuntime) {
    const session = state.snapshot.session || {};
    const activeNode = state.snapshot.activeNode || null;
    const phaseCurrent = String(session?.phase?.current || "").trim();
    const phaseStatus = String(session?.phase?.status || "").trim().toLowerCase();
    if (!phaseCurrent || !phaseStatus) {
      return buildBlockResult(
        "sy-pretool-write/gate1-phase",
        "运行态 phase 信息缺失，禁止写入生产代码",
        [
          `file: ${filePath}`,
          "要求：session.phase.current 与 session.phase.status 必须完整。",
        ],
      );
    }
    if (phaseStatus === "completed") {
      return buildBlockResult(
        "sy-pretool-write/gate1-phase",
        "当前 phase 已完成，禁止继续写入生产代码",
        [
          `file: ${filePath}`,
          `phase: ${phaseCurrent} (${phaseStatus})`,
        ],
      );
    }
    if (!session?.node || session.node.active_id === undefined || session.node.active_id === null) {
      return buildBlockResult(
        "sy-pretool-write/gate1-node",
        "运行态 active node 信息缺失，禁止写入生产代码",
        [
          `file: ${filePath}`,
          "要求：session.node.active_id 必须存在。",
        ],
      );
    }
    if (!activeNode) {
      return buildBlockResult(
        "sy-pretool-write/gate1-node",
        "当前没有 active node，禁止写入生产代码",
        [
          `file: ${filePath}`,
          `phase: ${phaseCurrent} (${phaseStatus})`,
          "要求：先进入有效 node 再写入生产代码。",
        ],
      );
    }
    const nodeStatus = String(activeNode.status || "").toLowerCase();
    if (!["in_progress", "blocked", "review"].includes(nodeStatus)) {
      return buildBlockResult(
        "sy-pretool-write/gate1-node",
        "当前 node 状态不允许写入生产代码",
        [
          `file: ${filePath}`,
          `node: ${activeNode.id || "unknown"} (${nodeStatus || "unknown"})`,
          "允许状态：in_progress | blocked | review。",
        ],
      );
    }
    if (activeNode.phase_id && phaseCurrent && activeNode.phase_id !== phaseCurrent) {
      return buildBlockResult(
        "sy-pretool-write/gate1-phase",
        "node.phase_id 与 session.phase.current 不一致，禁止写入生产代码",
        [
          `file: ${filePath}`,
          `node.phase_id: ${activeNode.phase_id}`,
          `session.phase.current: ${phaseCurrent}`,
        ],
      );
    }
  }

  let scopeDrift = false;
  if (shouldValidateRuntime) {
    const activeNode = state.snapshot.activeNode || null;
    const target = normalizeScopeTarget(activeNode?.target);
    if (target) {
      const inside = isInsideTarget(filePath, target, cwd);
      if (inside === false) {
        scopeDrift = true;
      }
    }
  }

  if (cfg.tddGateEnabled !== false && filePath && isProductionCode(filePath)) {
    if (state.runtimeReady && state.snapshot && state.snapshot.activeNode) {
      const activeNode = state.snapshot.activeNode;
      const tddState = String(activeNode.tdd_state || state.fields.node_state || "").toLowerCase();
      if (activeNode.tdd_required === true) {
        const tddException = policyContext?.tddException || null;
        if (tddException) {
          const missing = [];
          if (!tddException.reason) {
            missing.push("reason");
          }
          if (!tddException.alternative_verification) {
            missing.push("alternative_verification");
          }
          if (!tddException.user_approved) {
            missing.push("user_approved=true");
          }
          if (missing.length > 0) {
            return buildBlockResult(
              "sy-pretool-write/gate2-tdd-exception",
              "TDD 例外记录不完整或未获批准，禁止写入生产代码",
              [
                filePath ? `文件：${filePath}` : "",
                `节点：${activeNode.id || "unknown"}`,
                `缺少字段：${missing.join(", ")}`,
                "要求：补齐 reason / alternative_verification，并设置 user_approved=true。",
              ].filter(Boolean),
            );
          }
        }
        const tddExceptionApproved = Boolean(
          tddException
          && tddException.user_approved === true
          && tddException.reason
          && tddException.alternative_verification,
        );
        if (!tddExceptionApproved) {
          if (!V4_RED_READY_STATES.has(tddState)) {
            return blockForMissingRed(filePath, activeNode.id, tddState);
          }

          const verdict = evaluatePolicy({
            session: state.snapshot.session,
            taskGraph: state.snapshot.taskGraph,
            actionContext: policyContext.actionContext,
            specs: policyContext.specs,
          });

          if (policyContext.redEvidence && (verdict.primary_reason === "invalid_red" || verdict.test_gates?.pre_write_block === true)) {
            return blockForInvalidRed(filePath, activeNode.id, policyContext.redEvidence);
          }
        }
      }
    } else if (state.exists && state.phase === "execute" && state.fields) {
      const tddRequired = String(state.fields.tdd_required || "").toLowerCase();
      const redVerified = String(state.fields.red_verified || "").toLowerCase();
      if (tddRequired === "true" && redVerified !== "true") {
        return buildBlockResult(
          "sy-pretool-write/gate2-tdd",
          "TDD red gate: tdd_required=true but red_verified is not set to true",
          [
            `file: ${filePath}`,
            "Write and run a FAILING test first, then set red_verified=true in session state.",
            "See: sy-constraints/testing (Iron Law — write the failing test first)",
          ],
        );
      }
    }
  }

  if (policyContext) {
    const verdict = evaluatePolicy({
      session: state.snapshot?.session || {},
      taskGraph: state.snapshot?.taskGraph || {},
      actionContext: policyContext.actionContext,
      specs: policyContext.specs,
    });

    if (verdict.approval?.required === true && verdict.approval?.resolved !== true) {
      const approvalLines = buildApprovalLines(policyContext.targetPath, verdict);
      if (scopeDrift) {
        approvalLines.push(`注意：写入路径不在当前 node.target 范围内 (${state.snapshot?.activeNode?.target || "unknown"})。`);
      }
      return buildBlockResult(
        "sy-pretool-write/gate3-approval",
        "需要人工审批后才能继续写入",
        approvalLines,
      );
    }
  }

  if (scopeDrift) {
    return buildBlockResult(
      "sy-pretool-write/gate3-scope",
      "写入路径超出当前 node.target 约束范围",
      [
        `file: ${filePath}`,
        `target: ${state.snapshot?.activeNode?.target || "unknown"}`,
        "如需调整范围：请先更新 node.target 或通过检查点声明范围扩展。",
      ],
    );
  }

  if (content) {
    const result = scanSecrets(content, policy.secrets);
    if (result.blocked) {
      return buildBlockResult(
        "sy-pretool-write/gate4-secrets",
        `secret pattern detected: ${result.name}`,
        [
          filePath ? `file: ${filePath}` : "",
          "Use environment variables instead: process.env.SECRET / os.environ['KEY'] / std::env::var(\"KEY\")",
          "See: sy-constraints/appsec (secrets never in code)",
        ].filter(Boolean),
      );
    }
  }

  if (content && filePath && isProductionCode(filePath) && !/\.(md|yaml|yml|json|toml)$/i.test(filePath)) {
    const placeholderRe = /^\+?.*\b(TODO|FIXME|HACK|unimplemented!\s*\(|todo!\s*\(|raise\s+NotImplementedError|panic!\s*\("not\s+implemented)/m;
    const trackedRe = /TODO\s*\(\s*[^)]*\)\s*:?\s*#\d+|TODO\s*\([^)]*#\d+/;
    if (placeholderRe.test(content) && !trackedRe.test(content)) {
      return buildBlockResult(
        "sy-pretool-write/gate5-placeholder",
        "incomplete implementation: placeholder/stub marker in production code",
        [
          filePath ? `file: ${filePath}` : "",
          "Remove TODO/FIXME/HACK/unimplemented!() before writing.",
          "If deferring intentionally: add a tracked issue ref TODO(defer): #NNN",
          "and declare it as a new plan node in plan.md.",
          "See: executing-plans + verification-before-completion",
        ].filter(Boolean),
      );
    }
  }

  if (!isEvidenceFile && filePath) {
    const debugState = loadDebugState(state.fields);
    if (debugState.active && debugState.phase !== null && debugState.phase < 5) {
      return buildBlockResult(
        "sy-pretool-write/gate6-debug-iron-law",
        `debug Iron Law: cannot write source code at phase ${debugState.phase}/5`,
        [
          filePath ? `file: ${filePath}` : "",
          "Systematic debugging requires phases 1-4 to complete before any code change.",
          `Current: phase=${debugState.phase} hypotheses_tried=${debugState.hypothesisCount}`,
          debugState.nodeId ? `debug_node: ${debugState.nodeId}` : "",
          "Complete phases 1-4, then set debug_phase=5 to proceed.",
        ].filter(Boolean),
      );
    }
  }

  if (shouldCreatePreDestructiveCheckpoint(state, cwd, filePath)) {
    try {
      ensurePreDestructiveCheckpoint(cwd, {
        actor: "hook",
        nodeId: state.activeNodeId,
        phase: state.phaseId || state.phase || "none",
        targetRef: policyContext.targetPath,
        operationKind: String(payload.tool_name || payload.tool || "Write").toLowerCase().includes("edit") ? "edit" : "write",
        fileClass: policyContext.fileClass || null,
        sourceEvent: "pre_destructive_guard",
        metadata: {
          tool_name: String(payload.tool_name || payload.tool || "Write"),
        },
      });
    } catch (error) {
      return buildBlockResult(
        "sy-pretool-write/checkpoint",
        "pre-destructive checkpoint required before mutating an existing file",
        [
          filePath ? `file: ${filePath}` : "",
          `error: ${String(error.message || error)}`,
          "Resolve checkpoint creation before retrying the write.",
        ].filter(Boolean),
      );
    }
  }

  return buildAllowResult("allow_pretool_write");
}

function handlePretoolWriteSession(payload) {
  if (process.env.SY_BYPASS_SESSION_GUARD === "1" || process.env.SY_BYPASS_PRETOOL_WRITE === "1") {
    return buildAllowResult("bypass_session_guard");
  }

  const cwd = resolveCwd(payload);
  const filePath = resolveFilePath(payload);
  const content = resolveContent(payload);

  if (!isSessionFile(filePath)) {
    return buildAllowResult("non_session_file");
  }

  const isEdit = String(payload.tool_name || payload.tool || "").toLowerCase() === "edit";
  const legacyPhase = extractLegacyPhase(content);
  const v4PhaseStatus = extractNestedField(content, "phase", "status");

  if (legacyPhase !== null && !LEGAL_LEGACY_PHASES.has(legacyPhase)) {
    return buildBlockResult(
      "sy-pretool-write-session",
      `Invalid current_phase value: "${legacyPhase}"`,
      [
        `file: ${filePath}`,
        `Legal values: ${[...LEGAL_LEGACY_PHASES].filter(Boolean).join(" | ")}`,
        "Correct the legacy phase value before writing session.yaml.",
      ],
    );
  }

  if (v4PhaseStatus !== null && !LEGAL_V4_PHASE_STATUSES.has(v4PhaseStatus)) {
    return buildBlockResult(
      "sy-pretool-write-session",
      `Invalid phase.status value: "${v4PhaseStatus}"`,
      [
        `file: ${filePath}`,
        `Legal values: ${[...LEGAL_V4_PHASE_STATUSES].filter(Boolean).join(" | ")}`,
        "Correct the V4 phase.status value before writing session.yaml.",
      ],
    );
  }

  if (legacyPhase && !isEdit) {
    const { policy } = loadPolicy(cwd);
    const current = loadWorkflowState(cwd, policy);
    if (current.exists && current.phase && REGRESSION_MAP[current.phase] && REGRESSION_MAP[current.phase].has(legacyPhase)) {
      const onDiskRunId = String(asObject(current.fields).run_id || "").trim();
      const newRunId = extractRunId(content);
      if (!newRunId || newRunId === onDiskRunId) {
        warn(
          "sy-pretool-write-session",
          `Phase regression: ${current.phase} -> ${legacyPhase} (same run_id)`,
          [
            `file: ${filePath}`,
            "This usually means rework / rollback. If intentional, keep explicit evidence in the checkpoint.",
          ],
        );
      }
    }
  }

  const newRunId = extractRunId(content);
  if (newRunId && !RUN_ID_PATTERN.test(newRunId)) {
    return buildBlockResult(
      "sy-pretool-write-session",
      `Invalid run_id format: "${newRunId}"`,
      [
        `file: ${filePath}`,
        "Expected format: wf-YYYYMMDD-NNN (e.g. wf-20260307-001)",
      ],
    );
  }

  return buildAllowResult("allow_pretool_write_session");
}

function computeFingerprint(filePath) {
  try {
    const stats = fs.statSync(filePath);
    return `stat:${stats.size}-${Math.floor(stats.mtimeMs)}`;
  } catch {
    return "deleted";
  }
}

function updateIndex(indexPath, filePath, projectDir) {
  try {
    if (!fs.existsSync(indexPath)) {
      return;
    }
    const index = JSON.parse(fs.readFileSync(indexPath, "utf8"));
    const relPath = path.relative(projectDir, filePath).replace(/\\/g, "/");
    const files = asObject(index.files);
    if (!(relPath in files)) {
      return;
    }
    const entry = asObject(files[relPath]);
    entry.previous_fingerprint = entry.fingerprint;
    entry.fingerprint = computeFingerprint(filePath);
    entry.status = "MODIFIED";

    const understanding = asObject(entry.understanding);
    understanding.confidence = 0.0;
    const blindSpots = Array.isArray(understanding.blind_spots) ? understanding.blind_spots : [];
    blindSpots.push("File modified after last analysis — re-run /understand <path>.");
    understanding.blind_spots = blindSpots;
    entry.understanding = understanding;

    files[relPath] = entry;
    index.files = files;
    fs.writeFileSync(indexPath, JSON.stringify(index, null, 2), "utf8");
  } catch {
    // non-fatal
  }
}

function appendAudit(auditPath, entry) {
  try {
    fs.mkdirSync(path.dirname(auditPath), { recursive: true });
    fs.appendFileSync(auditPath, JSON.stringify(entry) + "\n", "utf8");
  } catch {
    // non-fatal
  }
}

function handlePosttoolWrite(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const filePath = resolveFilePath(payload);
  const content = resolveContent(payload);
  const toolName = String(payload.tool_name || payload.tool || "");
  const state = loadWorkflowState(cwd, policy);
  const fields = asObject(state.fields);
  const absoluteFilePath = filePath ? path.resolve(cwd, filePath) : "";
  const checkpointId = String(state.snapshot?.session?.recovery?.last_checkpoint_id || "").trim() || null;
  const isSessionFile = Boolean(filePath) && /\.ai[\\/]workflow[\\/]session\.(md|yaml)$/i.test(filePath.replace(/\\/g, "/"));
  let scopeDrift = false;
  let target = null;

  appendAudit(path.join(cwd, ".ai/workflow/audit.jsonl"), {
    ts: new Date().toISOString(),
    event: "PostToolUse",
    tool: toolName,
    file: filePath,
    fp: filePath ? computeFingerprint(absoluteFilePath) : null,
  });

  if (filePath) {
    updateIndex(
      path.join(cwd, ".ai/index.json"),
      absoluteFilePath,
      cwd,
    );
  }

  if (content) {
    for (const { name, regex } of MEDIUM_CONFIDENCE) {
      if (regex.test(content)) {
        warn(
          "sy-posttool-write",
          `possible credential pattern in write — verify it is env-var backed: ${name}`,
          [filePath ? `file: ${filePath}` : "", "See: sy-constraints/appsec"].filter(Boolean),
        );
        break;
      }
    }
  }

  if (isSessionFile && content) {
    const newNode = (content.match(/^[\s\-]*last_completed_node\s*:\s*(\S+)/im) || [])[1] || "";
    if (newNode && !/^(none|null|-)$/i.test(newNode)) {
      appendAudit(path.join(cwd, ".ai/workflow/audit.jsonl"), {
        ts: new Date().toISOString(),
        event: "VERIFY_PASS",
        node: newNode,
      });
    }
  }

  const isEvidenceFile = /\.(md|jsonl|json|yaml|yml|txt|log)$/i.test(filePath);
  if (!isEvidenceFile && filePath) {
    if (state.exists && state.phase === "execute" && state.fields) {
      target = String(state.fields.target || "").trim() || null;
      if (target) {
        const inside = isInsideTarget(filePath, target, cwd);
        if (inside === false) {
          scopeDrift = true;
          warn(
            "sy-posttool-write/scope-drift",
            "file written outside current_node.target",
            [
              `written:  ${filePath}`,
              `target:   ${target}`,
              "If intentional: declare this as scope change in checkpoint.",
              "See: executing-plans/operations/execute-node.md Step 4 (self-reflection)",
            ],
          );
        }
      }
    }
  }

  if (state.exists && fields.run_id && filePath) {
    appendEvent(cwd, {
      runId: String(fields.run_id || "").trim(),
      event: "write_recorded",
      phase: state.phaseId || state.phase || "none",
      nodeId: state.activeNodeId || String(fields.active_node_id || fields.current_node_id || fields.id || "").trim() || "none",
      actor: "hook",
      payload: {
        tool: toolName || "Write",
        file: filePath.replace(/\\/g, "/"),
        fingerprint: computeFingerprint(absoluteFilePath),
        checkpoint_id: checkpointId,
        scope_drift: scopeDrift,
        target,
        session_file: isSessionFile,
      },
    });
  }

  return buildAllowResult("allow_posttool_write");
}

function classifyPhase(command) {
  for (const { phase, patterns } of PHASE_CLASSIFIERS) {
    if (patterns.some((pattern) => pattern.test(command))) {
      return phase;
    }
  }
  return null;
}

function commandMatches(command, expected) {
  const normalizedExpected = normalizeCommand(expected);
  if (!normalizedExpected) {
    return false;
  }
  if (command === normalizedExpected) {
    return true;
  }
  return command.includes(normalizedExpected);
}

function extractKeySignal(stdout, stderr, exitCode) {
  const combined = [stdout, stderr].join("\n");
  const lines = combined.split("\n");
  const sample = [...lines.slice(0, 5), ...lines.slice(-20)].join("\n");

  if (exitCode === 0) {
    for (const pattern of PASS_SIGNALS) {
      const match = sample.match(pattern);
      if (match) {
        return match[0].trim().slice(0, 100);
      }
    }
    return "exit 0";
  }

  for (const pattern of FAIL_SIGNALS) {
    const match = sample.match(pattern);
    if (match) {
      return match[0].trim().slice(0, 100);
    }
  }
  const errLine = lines.find((line) => /error|FAIL/i.test(line) && line.trim());
  return errLine ? errLine.trim().slice(0, 100) : `exit ${exitCode}`;
}

function classifyFailureKind(stdout, stderr, exitCode) {
  if (exitCode === 0) {
    return "unexpected_pass";
  }
  const combined = `${stdout || ""}\n${stderr || ""}`.toLowerCase();
  for (const entry of FAILURE_KIND_PATTERNS) {
    if (entry.patterns.some((pattern) => pattern.test(combined))) {
      return entry.kind;
    }
  }
  return "environment_error";
}

function readStaging(stagingPath) {
  try {
    if (!fs.existsSync(stagingPath)) {
      return {};
    }
    return JSON.parse(fs.readFileSync(stagingPath, "utf8"));
  } catch {
    return {};
  }
}

function writeStaging(stagingPath, data) {
  try {
    fs.mkdirSync(path.dirname(stagingPath), { recursive: true });
    fs.writeFileSync(stagingPath, JSON.stringify(data, null, 2), "utf8");
  } catch {
    // non-fatal
  }
}

function syncToReport(reportPath, phase, phaseEntry) {
  try {
    if (!fs.existsSync(reportPath)) {
      return;
    }
    const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
    if (!report.verification) {
      return;
    }

    const status = phaseEntry.status;
    const updateObject = (fieldName) => {
      report.verification[fieldName] = {
        status,
        command: phaseEntry.command,
        exit_code: phaseEntry.exit_code,
        key_signal: phaseEntry.key_signal,
        source: "verify-staging",
        recorded_at: phaseEntry.ts,
      };
    };

    if (phase === "build") {
      updateObject("build");
    }
    if (phase === "typecheck") {
      updateObject("typecheck");
      report.verification.compile = status;
    }
    if (phase === "lint") {
      updateObject("lint");
    }
    if (phase === "test") {
      report.verification.tests = [{
        status,
        command: phaseEntry.command,
        exit_code: phaseEntry.exit_code,
        key_signal: phaseEntry.key_signal,
        source: "verify-staging",
        recorded_at: phaseEntry.ts,
      }];
      report.verification.test = status;
    }
    if (phase === "security") {
      updateObject("security");
    }

    report.updated_at = new Date().toISOString();

    const verif = report.verification;
    const hasFail = [
      verif.build?.status,
      verif.typecheck?.status,
      verif.lint?.status,
      Array.isArray(verif.tests) && verif.tests.some((entry) => entry?.status === "fail") ? "fail" : verif.test,
      verif.security?.status,
      verif.compile,
    ].some((value) => value === "fail");
    if (hasFail) {
      report.overall = "NOT_READY";
    }

    if (Array.isArray(report.evidence)) {
      const idx = report.evidence.findIndex((entry) => entry.phase === phase);
      const entry = { phase, command: phaseEntry.command, exit_code: phaseEntry.exit_code, signal: phaseEntry.key_signal };
      if (idx >= 0) {
        report.evidence[idx] = entry;
      } else {
        report.evidence.push(entry);
      }
    }

    fs.writeFileSync(reportPath, JSON.stringify(report, null, 2), "utf8");
  } catch {
    // non-fatal
  }
}

function handlePosttoolBash(payload) {
  if (process.env["SY_BYPASS_VERIFY_CAPTURE"] === "1") {
    return buildAllowResult("bypass_verify_capture");
  }

  const rawInput = resolveInput(payload);
  const command = String(rawInput.command || rawInput.cmd || "").trim();
  if (!command) {
    return buildAllowResult("empty_command");
  }

  const response = asObject(payload.tool_response ?? payload.tool_result ?? {});
  const exitCode = Number(response.returncode ?? response.exit_code ?? response.exitCode ?? -1);
  const stdout = String(response.stdout || "");
  const stderr = String(response.stderr || "");
  const keySignal = extractKeySignal(stdout, stderr, exitCode);
  const normalizedCommand = normalizeCommand(command);
  const phase = classifyPhase(command);

  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const state = loadWorkflowState(cwd, policy);
  const fields = asObject(state.fields);
  const runId = String(fields.run_id || "").trim();
  const nodeId = state.activeNodeId || String(fields.active_node_id || fields.current_node_id || fields.id || "").trim();
  const checkpointId = String(state.snapshot?.session?.recovery?.last_checkpoint_id || "").trim() || null;

  const stagingPath = path.join(cwd, ".ai/analysis/verify-staging.json");
  const reportPath = path.join(cwd, ".ai/analysis/ai.report.json");

  const testContract = state.snapshot?.activeNode?.test_contract || null;
  const redCmd = String(testContract?.red_cmd || "").trim();
  const greenCmd = String(testContract?.green_cmd || "").trim();
  const redMatched = redCmd ? commandMatches(normalizedCommand, redCmd) : false;
  const greenMatched = greenCmd ? commandMatches(normalizedCommand, greenCmd) : false;

  if (runId && redMatched) {
    appendEvent(cwd, {
      runId,
      event: "red_recorded",
      phase: state.phaseId || state.phase || "none",
      nodeId: nodeId || "none",
      actor: "hook",
      payload: {
        executed: true,
        testFailed: exitCode !== 0,
        failureKind: classifyFailureKind(stdout, stderr, exitCode),
        exitCode,
        recorded: true,
        command,
        key_signal: keySignal,
        checkpoint_id: checkpointId,
      },
    });
  }

  if (runId && greenMatched) {
    appendEvent(cwd, {
      runId,
      event: "green_recorded",
      phase: state.phaseId || state.phase || "none",
      nodeId: nodeId || "none",
      actor: "hook",
      payload: {
        executed: true,
        passed: exitCode === 0,
        newBlockerIntroduced: exitCode !== 0,
        exitCode,
        recorded: true,
        command,
        key_signal: keySignal,
        checkpoint_id: checkpointId,
      },
    });
  }

  if (!phase) {
    return buildAllowResult("non_verification_command");
  }

  const phaseEntry = {
    command,
    exit_code: exitCode,
    status: exitCode === 0 ? "pass" : "fail",
    key_signal: keySignal,
    ts: new Date().toISOString(),
    node: nodeId || undefined,
  };

  const staging = readStaging(stagingPath);
  if (!staging.phases) {
    staging.phases = {};
  }
  if (runId && staging.session_run_id && staging.session_run_id !== runId) {
    staging.phases = {};
  }
  staging.phases[phase] = phaseEntry;
  staging.updated_at = phaseEntry.ts;
  staging.session_run_id = runId || staging.session_run_id;
  writeStaging(stagingPath, staging);

  syncToReport(reportPath, phase, phaseEntry);

  if (runId) {
    appendEvent(cwd, {
      runId,
      event: "verification_recorded",
      phase: state.phaseId || state.phase || "none",
      nodeId: nodeId || "none",
      actor: "hook",
      payload: {
        verification_phase: phase,
        command,
        exit_code: exitCode,
        status: phaseEntry.status,
        key_signal: phaseEntry.key_signal,
        checkpoint_id: checkpointId,
        staging_ref: ".ai/analysis/verify-staging.json",
        report_synced: fs.existsSync(reportPath),
      },
    });
  }

  return buildAllowResult("allow_posttool_bash");
}

function gitContext(cwd) {
  try {
    const branch = execSync("git branch --show-current", { cwd, stdio: ["pipe", "pipe", "ignore"] })
      .toString().trim() || "unknown";
    const dirty = execSync("git status --porcelain", { cwd, stdio: ["pipe", "pipe", "ignore"] })
      .toString().trim().split("\n").filter(Boolean).length;
    return `GIT: branch=${branch}  dirty_files=${dirty}`;
  } catch {
    return null;
  }
}

function workflowContext(state) {
  if (!state.exists || state.isDone || state.isStale) {
    return null;
  }
  if (!state.phase && !state.nextAction) {
    return null;
  }
  const lines = [
    "ACTIVE WORKFLOW:",
    `  phase=${state.phase || "(unknown)"}  next_action=${state.nextAction || "(unknown)"}`,
    "  Run `工作流 继续` to resume or `工作流 状态` to inspect.",
  ];
  return lines.join("\n");
}

function buildBootstrap(policy, extras) {
  const ss = asObject(policy.sessionStart);
  const wf = String(ss.workflowSkill || "sy-workflow");
  const con = String(ss.constraintsSkill || "sy-constraints");
  const max = Number.isFinite(Number(ss.maxChildSkillsPerTurn))
    ? Number(ss.maxChildSkillsPerTurn)
    : 2;

  const lines = [
    "<SY-BOOTSTRAP>",
    "If there is even a 1% chance a sy-* skill applies, invoke the relevant skill first.",
    `Route via \`${wf}\`. Load baseline constraints via \`${con}\`.`,
    `Load child constraint skills minimally — baseline + at most ${max} task-specific children per turn`,
    "unless an incident or security escalation is active.",
    "Hooks enforce hard guards (dangerous commands, secrets, completion claims).",
    "Do NOT implement first and backfill constraints later. Constraints are pre-conditions.",
    "</SY-BOOTSTRAP>",
    ...extras.filter(Boolean),
  ];

  return lines.join("\n");
}

function handleSessionStart(payload) {
  const cwd = String(payload.cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd());
  const { policy } = loadPolicy(cwd);
  const ss = asObject(policy.sessionStart);

  if (ss.enabled === false) {
    return buildAllowResult("session_start_disabled");
  }

  const extras = [
    gitContext(cwd),
    workflowContext(loadWorkflowState(cwd, policy)),
    fs.existsSync(path.join(cwd, ".ai/index.json"))
      ? null
      : "INDEX: .ai/index.json not found — run `/init` before any development task.",
  ];

  const bootstrap = buildBootstrap(policy, extras);

  return buildAllowResult(
    "session_start",
    [],
    null,
    {
      additional_context: bootstrap,
      hookSpecificOutput: {
        hookEventName: "SessionStart",
        additionalContext: bootstrap,
      },
    },
  );
}

function handlePromptRefresh(payload) {
  const cwd = String(payload.cwd || process.env.CLAUDE_PROJECT_DIR || process.cwd());
  const { policy } = loadPolicy(cwd);
  const pr = asObject(policy.promptRefresh);

  if (process.env[String(pr.bypassEnv || "SY_BYPASS_PROMPT_REFRESH")] === "1") {
    return buildAllowResult("prompt_refresh_bypass");
  }

  const state = loadWorkflowState(cwd, policy);
  const activePhases = Array.isArray(pr.activePhases) ? pr.activePhases : ["plan", "execute", "review"];
  if (!state.exists || !activePhases.includes(state.phase)) {
    return buildAllowResult("prompt_refresh_inactive");
  }

  const prompt = String(payload.prompt || payload.message || "").toLowerCase();
  const keywords = Array.isArray(pr.triggerKeywords) ? pr.triggerKeywords : [];
  const matches = keywords.some((kw) => prompt.includes(String(kw).toLowerCase()));
  if (!matches) {
    return buildAllowResult("prompt_refresh_no_match");
  }

  const anchor = [
    `[sy-constraints] Active workflow phase: ${state.phase}.`,
    "Load relevant child constraint skill BEFORE writing any code.",
    "Hooks enforce dangerous-command and secrets guards. Do not pre-empt them.",
  ].join(" ");

  return buildAllowResult(
    "prompt_refresh",
    [],
    null,
    { context: anchor },
  );
}

function buildToolSelectionOutput(toolConfig) {
  return {
    hookSpecificOutput: {
      hookEventName: "BeforeToolSelection",
      toolConfig,
    },
  };
}

function shouldRestrictToolSelection(state, policy) {
  if (!state || !state.exists) {
    return false;
  }
  if (state.parseError && policy.restrictOnParseError !== false) {
    return true;
  }
  if (state.phase && state.phase !== "execute" && policy.restrictOnNonExecute !== false) {
    return true;
  }
  return false;
}

function handleBeforeToolSelection(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const cfg = asObject(policy.beforeToolSelection);
  if (cfg.enabled === false) {
    return buildAllowResult("before_tool_selection_disabled");
  }

  const bypassEnv = String(cfg.bypassEnv || "SY_BYPASS_TOOL_SELECTION");
  if (process.env[bypassEnv] === "1") {
    return buildAllowResult("before_tool_selection_bypass");
  }

  const state = loadWorkflowState(cwd, policy);
  const approvalPending = state.runtimeReady && isApprovalPending(state.snapshot);
  const restorePending = state.runtimeReady && isRestorePending(state.snapshot);
  const blockOnApproval = cfg.blockOnApprovalPending !== false;
  const blockOnRestore = cfg.blockOnRestorePending !== false;

  if ((approvalPending && blockOnApproval) || (restorePending && blockOnRestore)) {
    return buildAllowResult(
      "before_tool_selection_blocked",
      [],
      { approval_pending: approvalPending, restore_pending: restorePending },
      buildToolSelectionOutput({ mode: String(cfg.denyMode || "NONE").toUpperCase() }),
    );
  }

  if (shouldRestrictToolSelection(state, cfg)) {
    const allowedFunctionNames = normalizeStringList(cfg.allowedFunctionNames);
    const toolConfig = {
      mode: String(cfg.allowMode || "AUTO").toUpperCase(),
      ...(allowedFunctionNames.length > 0 ? { allowedFunctionNames } : {}),
    };
    return buildAllowResult(
      "before_tool_selection_read_only",
      [],
      { phase: state.phase || "" },
      buildToolSelectionOutput(toolConfig),
    );
  }

  return buildAllowResult("before_tool_selection_allow");
}

function redactSecretText(text, secretPolicy, options = {}) {
  let updated = String(text || "");
  let redacted = false;
  const findings = [];
  const maxRedactions = Number.isFinite(Number(options.maxRedactions)) ? Number(options.maxRedactions) : 5;
  for (let attempt = 0; attempt < maxRedactions; attempt += 1) {
    const hit = scanSecrets(updated, secretPolicy);
    if (!hit.blocked) {
      break;
    }
    const match = String(hit.match || "");
    if (!match) {
      break;
    }
    updated = updated.replace(match, "[REDACTED]");
    findings.push(String(hit.name || "secret"));
    redacted = true;
  }
  return { updated, redacted, findings };
}

function applyRedactionResult(result, tracker) {
  if (!result.redacted) {
    return;
  }
  tracker.redactionCount += 1;
  result.findings.forEach((entry) => tracker.redactionTypes.add(entry));
}

function redactTextField(target, key, secretPolicy, tracker, options = {}) {
  if (!target || typeof target !== "object") {
    return;
  }
  if (typeof target[key] !== "string") {
    return;
  }
  const result = redactSecretText(target[key], secretPolicy, options);
  if (result.redacted) {
    target[key] = result.updated;
    applyRedactionResult(result, tracker);
  }
}

function redactParts(parts, secretPolicy, tracker, options = {}) {
  if (!Array.isArray(parts)) {
    return;
  }
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    if (typeof part === "string") {
      const result = redactSecretText(part, secretPolicy, options);
      if (result.redacted) {
        parts[index] = result.updated;
        applyRedactionResult(result, tracker);
      }
      continue;
    }
    if (part && typeof part === "object" && typeof part.text === "string") {
      const result = redactSecretText(part.text, secretPolicy, options);
      if (result.redacted) {
        part.text = result.updated;
        applyRedactionResult(result, tracker);
      }
    }
  }
}

function redactLlmResponse(response, secretPolicy, options = {}) {
  const snapshot = JSON.parse(JSON.stringify(response));
  const tracker = {
    redactionCount: 0,
    redactionTypes: new Set(),
  };

  redactTextField(snapshot, "text", secretPolicy, tracker, options);

  if (Array.isArray(snapshot.candidates)) {
    for (const candidate of snapshot.candidates) {
      if (candidate && typeof candidate === "object") {
        redactTextField(candidate, "text", secretPolicy, tracker, options);
        const content = candidate.content && typeof candidate.content === "object" ? candidate.content : null;
        if (content) {
          if (Array.isArray(content.parts)) {
            redactParts(content.parts, secretPolicy, tracker, options);
          } else if (typeof content.parts === "string") {
            const result = redactSecretText(content.parts, secretPolicy, options);
            if (result.redacted) {
              content.parts = result.updated;
              applyRedactionResult(result, tracker);
            }
          }
          redactTextField(content, "text", secretPolicy, tracker, options);
        }
      }
    }
  }

  if (snapshot.content && typeof snapshot.content === "object") {
    if (Array.isArray(snapshot.content.parts)) {
      redactParts(snapshot.content.parts, secretPolicy, tracker, options);
    } else if (typeof snapshot.content.parts === "string") {
      const result = redactSecretText(snapshot.content.parts, secretPolicy, options);
      if (result.redacted) {
        snapshot.content.parts = result.updated;
        applyRedactionResult(result, tracker);
      }
    }
    redactTextField(snapshot.content, "text", secretPolicy, tracker, options);
  }

  if (Array.isArray(snapshot.parts)) {
    redactParts(snapshot.parts, secretPolicy, tracker, options);
  }

  return {
    response: snapshot,
    redactionCount: tracker.redactionCount,
    redactionTypes: Array.from(tracker.redactionTypes),
  };
}

function handleAfterModel(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const afterModel = asObject(policy.afterModel);
  if (afterModel.enabled === false) {
    return buildAllowResult("after_model_disabled");
  }
  const secretPolicy = asObject(policy.secrets);
  const bypassEnv = String(afterModel.bypassEnv || secretPolicy.bypassEnv || "SY_BYPASS_SECRET_GUARD");
  if (process.env[bypassEnv] === "1") {
    return buildAllowResult("after_model_bypass");
  }

  const response = payload && typeof payload.llm_response === "object" && payload.llm_response !== null
    ? payload.llm_response
    : null;
  if (!response || Array.isArray(response) || Object.keys(response).length === 0) {
    return buildAllowResult("after_model_no_response");
  }

  const maxRedactions = Number.isFinite(Number(afterModel.maxRedactions))
    ? Number(afterModel.maxRedactions)
    : 5;
  const redaction = redactLlmResponse(response, secretPolicy, { maxRedactions });
  if (redaction.redactionCount === 0) {
    return buildAllowResult("after_model_allow");
  }

  return buildAllowResult(
    "after_model_redacted",
    [],
    { redaction_count: redaction.redactionCount, redaction_types: redaction.redactionTypes },
    {
      hookSpecificOutput: {
        hookEventName: "AfterModel",
        llm_response: redaction.response,
      },
    },
  );
}

function classifyMutatingCommand(command) {
  if (/\bgit\s+(add|commit|push|rm|mv|merge|rebase|cherry-pick|restore|checkout\s+\S+[\s\S]*\s--\s+\S+|clean|reset|stash\s+(pop|apply))\b/i.test(command)) {
    return "git_mutating";
  }
  if (/(^|[;&|\n])\s*(rm\s+-r|rm\s+-f|mv\s+|cp\s+-f|sed\s+-i\b|perl\s+-pi\b|del\s+\/f\b|rd\s+\/s\s+\/q\b)/i.test(command)) {
    return "destructive";
  }
  return null;
}

function normalizeCommand(command) {
  return String(command || "").trim().replace(/\s+/g, " ");
}

function matchCommandClass(command, classification) {
  const classes = asObject(classification?.classes);
  const priority = Array.isArray(classification?.priority) && classification.priority.length > 0
    ? classification.priority
    : Object.keys(classes);
  for (const classId of priority) {
    const entry = asObject(classes[classId]);
    const patterns = Array.isArray(entry.patterns) ? entry.patterns : [];
    for (const pattern of patterns) {
      const regex = compilePattern(asObject(pattern).regex, "i");
      if (regex && regex.test(command)) {
        return classId;
      }
    }
  }
  return null;
}

function classifyCommand(command) {
  const normalized = normalizeCommand(command);
  if (/\bgit\s+commit\b/i.test(normalized) && /\b--dry-run\b/i.test(normalized)) {
    return "safe";
  }
  if (/\bgit\s+push\b/i.test(normalized) && /\b--dry-run\b/i.test(normalized)) {
    return "safe";
  }
  const classification = getWorkflowSpecs().hookSpec?.command_classification;
  const fromSpec = classification ? matchCommandClass(normalized, classification) : null;
  return fromSpec || classifyMutatingCommand(normalized) || "safe";
}

function deriveCommandChangeClass(commandClass) {
  if (commandClass === "verify" || commandClass === "safe") {
    return "utility";
  }
  return "feature";
}

function hasApprovedCommandGrant(snapshot, command, commandClass) {
  const grants = Array.isArray(snapshot?.session?.approvals?.grants) ? snapshot.session.approvals.grants : [];
  const normalizedCommand = normalizeCommand(command);
  return grants.some((grant) => {
    if (String(grant?.decision || "").toLowerCase() !== "approved") {
      return false;
    }
    const action = String(grant?.action || "").toLowerCase();
    if (action && !["bash", "command", "run_command", "shell"].includes(action)) {
      return false;
    }
    const targetRef = String(grant?.target_ref || "").trim();
    return [normalizedCommand, commandClass, "*"].includes(targetRef);
  });
}

function buildBashApprovalLines(command, verdict) {
  const approval = verdict.approval || {};
  return buildShortApprovalRequest({
    actionLabel: "执行命令",
    targetRef: `${command} (${approval.command_class || "unknown"})`,
    riskClass: approval.risk_class || "unknown",
    approvalMode: approval.approval_mode || "manual_required",
    grantScopes: Array.isArray(approval.allowed_grant_scopes) && approval.allowed_grant_scopes.length > 0
      ? approval.allowed_grant_scopes
      : ["once"],
  });
}

function handlePretoolBash(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const cfg = asObject(policy.pretoolBash);

  const bypass = String(cfg.bypassEnv || "SY_BYPASS_PRETOOL_BASH");
  if (process.env[bypass] === "1") {
    return buildAllowResult("bypass_pretool_bash");
  }

  const toolName = String(payload.tool_name || payload.tool || "").toLowerCase();
  if (!/(bash|shell|command)/i.test(toolName)) {
    return buildAllowResult("non_bash_tool");
  }

  const input = resolveInput(payload);
  const command = String(input.command || input.cmd || "").trim();
  if (!command) {
    return buildAllowResult("empty_command");
  }

  const blocked = Array.isArray(cfg.blockedCommands) ? cfg.blockedCommands : [];
  for (const item of blocked) {
    const rule = asObject(item);
    const regex = compilePattern(rule.regex, "i");
    if (!regex || !regex.test(command)) {
      continue;
    }
    const reason = String(rule.reason || "blocked by policy");
    const cites = String(rule.cites || "sy-constraints/safety");
    return buildBlockResult(
      "sy-pretool-bash",
      reason,
      [`command: ${command}`, `see: ${cites}`],
    );
  }

  const commitAllowEnv = String(cfg.commitAllowEnv || "SY_ALLOW_GIT_COMMIT");
  const commitAuthorized = /\bgit\s+commit\b/i.test(command) && process.env[commitAllowEnv] === "1";
  if (
    /\bgit\s+commit\b(?![\s\S]*\b--dry-run\b)/i.test(command) &&
    process.env[commitAllowEnv] !== "1"
  ) {
    return buildBlockResult(
      "sy-pretool-bash",
      `git commit requires explicit session authorization — set ${commitAllowEnv}=1`,
      [
        `command: ${command}`,
        "Stage changes and ask the user to review before committing.",
        "See: sy-constraints/execution (commit safety)",
      ],
    );
  }

  const pushAllowEnv = String(cfg.pushAllowEnv || "SY_ALLOW_GIT_PUSH");
  const pushAuthorized = /\bgit\s+push\b/i.test(command) && process.env[pushAllowEnv] === "1";
  if (/\bgit\s+push\b/i.test(command) && process.env[pushAllowEnv] !== "1") {
    return buildBlockResult(
      "sy-pretool-bash",
      `git push requires explicit session authorization — set ${pushAllowEnv}=1`,
      [
        `command: ${command}`,
        "Inform the user what will be pushed and await confirmation.",
        "See: sy-constraints/execution (commit safety) + sy-constraints/safety",
      ],
    );
  }

  const commandClass = classifyCommand(command);
  const state = loadWorkflowState(cwd, policy);
  const specs = getWorkflowSpecs();
  const approvalGranted = Boolean(commitAuthorized || pushAuthorized || hasApprovedCommandGrant(state.snapshot, command, commandClass));
  const tddExceptionActive = Boolean(state.runtimeReady && normalizeTddException(state.snapshot?.activeNode?.tdd_exception));

  // Persona command permission check (Batch 2.2)
  if (state.runtimeReady && state.snapshot) {
    const personaCmdBlock = checkPersonaCommandPermission(state.snapshot, commandClass, specs);
    if (personaCmdBlock?.blocked) {
      return buildBlockResult(
        "sy-pretool-bash/gate-persona",
        personaCmdBlock.reason,
        [`command: ${command}`, `persona: ${personaCmdBlock.persona}`, "see: workflow/persona-bindings.yaml"],
      );
    }
  }

  const verdict = evaluatePolicy({
    session: state.snapshot?.session || {},
    taskGraph: state.snapshot?.taskGraph || {},
    actionContext: {
      targetPath: command,
      commandClass,
      changeClass: deriveCommandChangeClass(commandClass),
      behaviorChange: false,
      verificationPassed: false,
      changeIsAuditable: true,
      approvalGranted,
      tddExceptionActive,
      touchesAuthOrSecurity: commandClass === "privileged",
      touchesSchemaOrPublicApi: commandClass === "schema_mutation",
      touchesDataMigration: commandClass === "data_mutation",
    },
    specs,
  });

  if (verdict.approval?.required === true && verdict.approval?.resolved !== true) {
    return buildBlockResult(
      "sy-pretool-bash/approval",
      "需要人工审批后才能继续执行命令",
      buildBashApprovalLines(command, verdict),
    );
  }

  if (["git_mutating", "destructive", "schema_mutation", "data_mutation", "privileged"].includes(commandClass)) {
    if (state.runtimeReady && state.activeNodeId && state.activeNodeId !== "none") {
      try {
        ensurePreDestructiveCheckpoint(cwd, {
          actor: "hook",
          nodeId: state.activeNodeId,
          phase: state.phaseId || state.phase || "none",
          targetRef: command,
          operationKind: "bash",
          commandClass,
          sourceEvent: "pre_destructive_guard",
          metadata: {
            tool_name: payload.tool_name || payload.tool || "Bash",
          },
        });
      } catch (error) {
        return buildBlockResult(
          "sy-pretool-bash/checkpoint",
          "pre-destructive checkpoint required before mutating command",
          [
            `command: ${command}`,
            `error: ${String(error.message || error)}`,
            "Resolve checkpoint creation before retrying the command.",
          ],
        );
      }
    }
  }

  return buildAllowResult("allow_pretool_bash");
}

function handlePretoolBashBudget(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const cfg = asObject(policy.pretoolBash);

  const budgetBypassEnv = "SY_BYPASS_LOOP_BUDGET";
  const bashBypassEnv = String(cfg.bypassEnv || "SY_BYPASS_PRETOOL_BASH");
  if (process.env[budgetBypassEnv] === "1" || process.env[bashBypassEnv] === "1") {
    return buildAllowResult("bypass_loop_budget");
  }

  const toolName = String(payload.tool_name || payload.tool || "").toLowerCase();
  if (!/(bash|shell|command)/i.test(toolName)) {
    return buildAllowResult("non_bash_tool");
  }

  const input = resolveInput(payload);
  const command = String(input.command || input.cmd || "").trim();
  if (!command) {
    return buildAllowResult("empty_command");
  }

  if (!looksLikeExecutionCommand(command)) {
    return buildAllowResult("non_execution_command");
  }

  const state = loadWorkflowState(cwd, policy);
  if (!state.exists || state.phase !== "execute") {
    return buildAllowResult("non_execute_phase");
  }

  if (state.runtimeReady && state.snapshot) {
    const session = asObject(state.snapshot.session);
    const budget = asObject(session.loop_budget);
    const approvals = asObject(session.approvals);
    const recovery = asObject(session.recovery);
    const recommendedNext = state.snapshot.sprintStatus?.recommended_next || [];
    const hits = [];

    const maxNodes = Number(budget.max_nodes);
    const consumedNodes = Number(budget.consumed_nodes);
    if (Number.isFinite(maxNodes) && Number.isFinite(consumedNodes) && consumedNodes >= maxNodes) {
      hits.push(`loop_budget.max_nodes exhausted: consumed_nodes=${consumedNodes}, max_nodes=${maxNodes}`);
    }

    const maxFailures = Number(budget.max_failures);
    const consumedFailures = Number(budget.consumed_failures);
    if (Number.isFinite(maxFailures) && Number.isFinite(consumedFailures) && consumedFailures >= maxFailures) {
      hits.push(`loop_budget.max_failures exhausted: consumed_failures=${consumedFailures}, max_failures=${maxFailures}`);
    }

    const maxPendingApprovals = Number(budget.max_pending_approvals);
    const pendingCount = Number(approvals.pending_count);
    if (Number.isFinite(maxPendingApprovals) && Number.isFinite(pendingCount) && pendingCount > maxPendingApprovals) {
      hits.push(`approval queue exceeded: pending_count=${pendingCount}, max_pending_approvals=${maxPendingApprovals}`);
    }

    if (approvals.pending === true) {
      hits.push(`approval_pending: active_request=${String(approvals.active_request?.approval_id || "unknown")}`);
    }

    if (recovery.restore_pending === true) {
      hits.push(`restore_pending: ${String(recovery.restore_reason || "restore_pending")}`);
    }

    if (hits.length === 0) {
      return buildAllowResult("budget_ok");
    }

    return buildBlockResult(
      "sy-pretool-bash-budget",
      "自动执行已被预算门阻断",
      [
        ...hits,
        `recommended_next: ${formatFirstRecommendedNext(recommendedNext)}`,
        "请先写入检查点，再按 recommended_next 恢复执行。",
        "如需临时绕过：SY_BYPASS_LOOP_BUDGET=1",
      ],
    );
  }

  const fields = asObject(state.fields);
  const mode = String(fields.mode || "").toLowerCase();

  if (!["auto", "batch", "parallel"].includes(mode)) {
    return buildAllowResult("non_loop_mode");
  }

  const maxNodes = Number(fields.loop_budget_max_nodes ?? 5);
  const maxMinutes = Number(fields.loop_budget_max_minutes ?? 30);
  const maxFailures = Number(fields.loop_budget_max_consecutive_failures ?? 2);
  const startedAtRaw = String(fields.loop_budget_started_at || "");
  const startedAtMs = Date.parse(startedAtRaw);

  if (!startedAtRaw) {
    return buildAllowResult("budget_uninitialized");
  }

  const auditPath = path.join(cwd, ".ai/workflow/audit.jsonl");
  const verifiedNodes = countVerifiedNodes(auditPath);
  const consecutiveFailures = countConsecutiveFailures(auditPath);
  const elapsedMs = Number.isFinite(startedAtMs) ? Date.now() - startedAtMs : 0;
  const elapsedMinutes = Math.floor(elapsedMs / 60_000);

  const hits = [];

  if (verifiedNodes >= maxNodes) {
    hits.push(`max_nodes: ${verifiedNodes}/${maxNodes} nodes verified`);
  }
  if (Number.isFinite(startedAtMs) && elapsedMinutes >= maxMinutes) {
    hits.push(`max_minutes: ${elapsedMinutes}/${maxMinutes} minutes elapsed`);
  }
  if (consecutiveFailures >= maxFailures) {
    hits.push(`max_consecutive_failures: ${consecutiveFailures} consecutive FAIL(s)`);
  }

  if (hits.length === 0) {
    return buildAllowResult("budget_ok");
  }

  return buildBlockResult(
    "sy-pretool-bash-budget",
    `自动执行预算已耗尽（mode=${mode}）`,
    [
      "",
      "预算命中：",
      ...hits.map((hit) => `  - ${hit}`),
      "",
      "处理要求：",
      "  1. 先写检查点，总结已完成节点与当前状态",
      "  2. 写入 session.yaml.next_action，指向下一节点或 all done",
      "  3. 等待人工确认后再继续",
      "",
      "如需临时绕过：SY_BYPASS_LOOP_BUDGET=1（仅当前会话）",
      "如需调整预算：先更新 session.yaml 中的 loop_budget_* 字段",
      "参考：executing-plans/SKILL.md — Loop Budget",
    ],
  );
}

function acquireStopLock(stalenessMs) {
  try {
    if (fs.existsSync(STOP_LOCK_PATH)) {
      const age = Date.now() - fs.statSync(STOP_LOCK_PATH).mtimeMs;
      if (age < stalenessMs) {
        return false;
      }
      fs.unlinkSync(STOP_LOCK_PATH);
    }
    fs.writeFileSync(STOP_LOCK_PATH, String(Date.now()), "utf8");
    return true;
  } catch {
    return true;
  }
}

function releaseStopLock() {
  try {
    fs.unlinkSync(STOP_LOCK_PATH);
  } catch {
    // noop
  }
}

function loadReport(reportPath) {
  try {
    if (!fs.existsSync(reportPath)) {
      return null;
    }
    return JSON.parse(fs.readFileSync(reportPath, "utf8"));
  } catch {
    return null;
  }
}

function reportIsStale(report, maxAgeHours) {
  const ts = Date.parse(String(
    asObject(report).updated_at || asObject(report).generated_at || "",
  ));
  if (!Number.isFinite(ts)) {
    return false;
  }
  return Date.now() - ts > maxAgeHours * 3_600_000;
}

function auditExists(cwd) {
  return fs.existsSync(path.join(cwd, ".ai/workflow/audit.jsonl"));
}

function verificationStates(report) {
  const v = asObject(asObject(report).verification);
  const stateOf = (value) => {
    if (typeof value === "string") {
      return String(value || "skip").toLowerCase();
    }
    if (Array.isArray(value)) {
      if (value.some((entry) => stateOf(entry) === "fail")) {
        return "fail";
      }
      if (value.some((entry) => stateOf(entry) === "pass")) {
        return "pass";
      }
      if (value.some((entry) => stateOf(entry) === "skip")) {
        return "skip";
      }
      return "skip";
    }
    if (value && typeof value === "object") {
      return stateOf(value.status || value.result || "skip");
    }
    return "skip";
  };
  return {
    compile: stateOf(v.compile || v.typecheck),
    test: stateOf(v.test || v.tests),
    lint: stateOf(v.lint),
    build: stateOf(v.build),
  };
}

function formatRecommendedNext(items) {
  const nextItems = Array.isArray(items) ? items : [];
  if (nextItems.length === 0) {
    return "(none)";
  }
  return nextItems
    .map((item) => {
      const current = asObject(item);
      const type = String(current.type || "unknown").trim();
      const target = String(current.target || "unknown").trim();
      return `${type}:${target}`;
    })
    .join(", ");
}

function getCompletedNodeIds(state) {
  if (state.runtimeReady && state.snapshot && Array.isArray(state.snapshot.taskGraph?.nodes)) {
    return state.snapshot.taskGraph.nodes
      .filter((node) => asObject(node).status === "completed")
      .map((node) => String(asObject(node).id || "").trim())
      .filter(Boolean);
  }

  const lastCompletedNode = String(state.fields?.last_completed_node || "").trim();
  if (!lastCompletedNode || lastCompletedNode.toLowerCase() === "none") {
    return [];
  }
  return [lastCompletedNode];
}

function numericOrNull(value) {
  const normalized = Number(value);
  return Number.isFinite(normalized) ? normalized : null;
}

function runtimeFieldsCompleteForStop(session, activePhase, activeNode) {
  if (!session || typeof session !== "object") {
    return false;
  }
  if (!session.run_id || !session?.phase?.current || !session?.phase?.status) {
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
  if (!activePhase) {
    return false;
  }
  if (!session?.timestamps?.updated_at) {
    return false;
  }
  const budget = session.loop_budget;
  if (!budget) {
    return false;
  }
  if (numericOrNull(budget.max_nodes) === null || numericOrNull(budget.max_failures) === null || numericOrNull(budget.max_pending_approvals) === null) {
    return false;
  }
  return true;
}

function approvalQueueExceeded(session) {
  const pendingCount = numericOrNull(session?.approvals?.pending_count);
  const maxPending = numericOrNull(session?.loop_budget?.max_pending_approvals);
  if (pendingCount === null || maxPending === null) {
    return false;
  }
  return pendingCount > maxPending;
}

function loopBudgetExceeded(session) {
  const consumedNodes = numericOrNull(session?.loop_budget?.consumed_nodes);
  const maxNodes = numericOrNull(session?.loop_budget?.max_nodes);
  const consumedFailures = numericOrNull(session?.loop_budget?.consumed_failures);
  const maxFailures = numericOrNull(session?.loop_budget?.max_failures);
  if (consumedNodes !== null && maxNodes !== null && consumedNodes > maxNodes) {
    return true;
  }
  if (consumedFailures !== null && maxFailures !== null && consumedFailures > maxFailures) {
    return true;
  }
  return false;
}

function handleStop(payload) {
  const cwd = resolveCwd(payload);
  const { policy } = loadPolicy(cwd);
  const cfg = asObject(policy.stop);

  if (process.env[String(cfg.bypassEnv || "SY_BYPASS_STOP_GUARD")] === "1") {
    return buildAllowResult("bypass_stop_guard");
  }

  const stalenessMs = Number(cfg.lockStalenessMs) || 30_000;
  if (!acquireStopLock(stalenessMs)) {
    return buildAllowResult("stop_lock_active");
  }

  try {
    return runStopGates(cwd, policy, cfg);
  } finally {
    releaseStopLock();
  }
}

function runStopGates(cwd, policy, cfg) {
  const state = loadWorkflowState(cwd, policy);
  const passPhases = Array.isArray(cfg.passPhases) ? cfg.passPhases : [
    "done", "explore", "exploring", "benchmark", "benchmarking",
    "free-ideation", "design", "designing", "brainstorm", "ideation", "",
  ];
  if (!state.exists || passPhases.includes(state.phase)) {
    return buildAllowResult("stop_pass_phase");
  }

  if (state.parseError) {
    return buildForceContinueResult(
      "sy-stop",
      "工作流状态解析失败，无法确认是否可以结束本轮",
      [
        `session: ${state.sessionPath}`,
        "请先修复 session 文件；如需临时绕过，可设置 SY_BYPASS_STOP_GUARD=1。",
        "参考：sy-constraints/execution",
      ],
    );
  }

  const reportPath = path.join(cwd, String(cfg.reportRelativePath || ".ai/analysis/ai.report.json"));
  const maxAgeHours = Number(cfg.maxReportAgeHours) || 6;

  if (state.runtimeReady && state.snapshot) {
    const snapshot = state.snapshot;
    const session = asObject(snapshot.session);
    const approvals = asObject(session.approvals);
    const recovery = asObject(session.recovery);
    const frontier = buildResumeFrontier(cwd);
    const recommendedNext = Array.isArray(frontier.recommended_next) ? frontier.recommended_next : [];

    if (frontier.recovery_required) {
      return buildForceContinueResult(
        "sy-stop",
        "恢复未完成，当前轮次不能结束",
        buildShortRestoreRequest({
          reason: recovery.restore_reason || frontier.reasons?.[0] || "restore_pending",
          checkpointId: frontier.last_checkpoint_id || "none",
          activeNode: frontier.active_node || state.activeNodeId || "none",
          targetRef: recommendedNext[0]?.target || state.activeNodeId || session.phase?.current || "runtime",
          recommendedNext,
        }),
      );
    }

    if (approvals.pending === true) {
      return buildForceContinueResult(
        "sy-stop",
        "存在待审批事项，当前轮次不能结束",
        buildShortApprovalRequest({
          actionLabel: String(approvals.active_request?.action || "待审批操作"),
          targetRef: String(approvals.active_request?.target_ref || recommendedNext[0]?.target || "unknown"),
          riskClass: String(approvals.active_request?.risk_class || "unknown"),
          approvalMode: String(approvals.active_request?.approval_mode || "manual_required"),
          grantScopes: [String(approvals.active_request?.grant_scope || "once")],
        }).concat([
          `审批编号：${String(approvals.active_request?.approval_id || "unknown")}`,
          `当前建议：${formatRecommendedNext(recommendedNext)}`,
        ]),
      );
    }
  }

  const failures = [];

  if (state.runtimeReady && state.snapshot) {
    const session = asObject(state.snapshot.session);
    const activePhase = state.snapshot.activePhase;
    const activeNode = state.snapshot.activeNode;
    if (!runtimeFieldsCompleteForStop(session, activePhase, activeNode)) {
      failures.push("runtime 状态字段不完整（phase/node/budget/timestamps）");
    }
    if (approvalQueueExceeded(session)) {
      failures.push("审批队列超过 max_pending_approvals");
    }
    if (loopBudgetExceeded(session)) {
      failures.push("循环预算耗尽（max_nodes 或 max_failures 已超限）");
    }
  }

  if (state.phase === "plan") {
    if (!state.fields?.updated_at) {
      failures.push("session.yaml missing updated_at — save session state before stopping");
    }
  }

  if (state.phase === "execute") {
    if (!auditExists(cwd)) {
      failures.push(
        "audit.jsonl not found — PostToolUse hook may not have run; " +
        "re-run with hooks enabled or set SY_BYPASS_STOP_GUARD=1",
      );
    }

    const lastNode = String(state.fields?.last_completed_node || "").trim();
    if (lastNode && lastNode.toLowerCase() !== "none") {
      const report = loadReport(reportPath);
      if (!report) {
        failures.push(
          `node '${lastNode}' is marked complete but ai.report.json is missing — ` +
          "run verification before claiming node done. See: sy-constraints/verify",
        );
      } else if (reportIsStale(report, maxAgeHours)) {
        failures.push(
          `node '${lastNode}' verification report is stale (>${maxAgeHours}h) — ` +
          "re-run verification. See: sy-constraints/verify",
        );
      } else {
        const states = verificationStates(report);
        if (Object.values(states).includes("fail")) {
          failures.push(
            `verification has FAIL state(s): ${JSON.stringify(states)} — ` +
            "fix failures before stopping. See: sy-constraints/verify",
          );
        }
        if (Object.values(states).every((value) => value === "skip")) {
          failures.push(
            "all verification checks are 'skip' — no PASS evidence for completed node. " +
            "See: sy-constraints/verify",
          );
        }
      }
    }

    if (!state.nextAction) {
      failures.push(
        "session.yaml missing next_action — set before stopping so work can resume. " +
        "See: sy-constraints/phase (checkpoint policy)",
      );
    }
  }

  if (state.phase === "review") {
    const report = loadReport(reportPath);
    if (!report) {
      failures.push(
        "ai.report.json not found — generate verification report before review stop. " +
        "See: sy-constraints/verify",
      );
    } else if (reportIsStale(report, maxAgeHours)) {
      failures.push(
        `ai.report.json is stale (>${maxAgeHours}h) — regenerate before completing review. ` +
        "See: sy-constraints/verify",
      );
    }

    const completedNodeIds = getCompletedNodeIds(state);
    if (completedNodeIds.length > 0) {
      const { nodeIds } = countLedgerNodes(cwd);
      const missingNodeIds = completedNodeIds.filter((nodeId) => !nodeIds.includes(nodeId));
      if (missingNodeIds.length > 0) {
        failures.push(
          `ledger coverage: ${completedNodeIds.length - missingNodeIds.length}/${completedNodeIds.length} completed nodes have ledger entries ? ` +
          `run /execute verify for node(s) missing from ledger. Missing: [${missingNodeIds.join(", ")}]. ` +
          `Verified: [${nodeIds.join(", ") || "none"}]. ` +
          "See: verification-before-completion (Step 3 ? ledger audit)",
        );
      }
    }
  }

  if (failures.length > 0) {
    return buildForceContinueResult(
      "sy-stop",
      "检查点未完成",
      [
        `phase: ${state.phase}  next_action: ${state.nextAction || "(未设置)"}`,
        ...failures.map((failure, index) => `  [${index + 1}] ${failure}`),
        "",
        "请先解决以上问题，再结束本轮；如需临时绕过，可设置 SY_BYPASS_STOP_GUARD=1。",
      ],
    );
  }

  if (state.hasPending) {
    return buildAllowResult(
      "stop_with_pending",
      [
        `[sy-stop] Session stop with active workflow: phase=${state.phase} next=${state.nextAction}.`,
        "检查点已通过。需要继续时，请执行 `工作流 继续`。",
      ],
    );
  }

  return buildAllowResult("stop_ok");
}

function dispatchHookEvent(event, payload) {
  const contractGuard = guardHookClientContracts(payload);
  if (contractGuard) {
    return contractGuard;
  }
  const normalized = String(event || "").trim();
  if (normalized === "SessionStart") {
    return handleSessionStart(payload);
  }
  if (normalized === "UserPromptSubmit") {
    return handlePromptRefresh(payload);
  }
  if (normalized === "BeforeToolSelection") {
    return handleBeforeToolSelection(payload);
  }
  if (normalized === "PreToolUse:Write|Edit") {
    return handlePretoolWrite(payload);
  }
  if (normalized === "PreToolUse:WriteSession") {
    return handlePretoolWriteSession(payload);
  }
  if (normalized === "PreToolUse:Bash") {
    return handlePretoolBash(payload);
  }
  if (normalized === "PreToolUse:BashBudget") {
    return handlePretoolBashBudget(payload);
  }
  if (normalized === "PostToolUse:Write|Edit") {
    return handlePosttoolWrite(payload);
  }
  if (normalized === "PostToolUse:Bash") {
    return handlePosttoolBash(payload);
  }
  if (normalized === "AfterModel") {
    return handleAfterModel(payload);
  }
  if (normalized === "Stop") {
    return handleStop(payload);
  }
  return buildAllowResult("unknown_hook_event");
}

async function runHookAndExit(event) {
  try {
    const raw = await readStdin();
    const payload = asObject(parseJsonSafe(raw, {}));
    const result = dispatchHookEvent(event, payload);
    emitResult(result);
  } catch (error) {
    emitResult(buildAllowResult(
      "hook_error",
      ["hook error — fail open", String(error?.message || error || "unknown")],
    ));
  }
}

module.exports = {
  persistOutputTemplatesForTest,
  dispatchHookEvent,
  runHookAndExit,
  buildAllowResult,
  buildBlockResult,
  buildForceContinueResult,
  buildInteractionEnvelope,
  getActiveNode,
  getActivePhase,
  getRecommendedNext,
  hasCompleteRuntime,
  isApprovalPending,
  isRestorePending,
  loadRuntimeSnapshot,
  projectWorkflowCompatState,
};






