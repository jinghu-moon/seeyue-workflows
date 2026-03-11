#!/usr/bin/env node
"use strict";

/**
 * sy-hook-lib.cjs — shared primitives for all sy-* hooks
 *
 * Fix log vs original:
 *   [P0] Policy array merge: blockedCommands / highConfidencePatterns /
 *        placeholderPatterns / expectedSkills are now CONCATENATED, not replaced.
 *        A project policy that adds one custom rule no longer silently drops all
 *        built-in rules.
 *   [P1] resolveContent(): unified Write|Edit content extraction. Previously each
 *        guard re-implemented this and missed Edit's `new_string` field.
 *   [P1] git_checkout_discard regex narrowed: was matching any
 *        `git checkout ... -- <path>` (legal) — now only matches
 *        `git checkout --` with no prior branch arg (actual discard).
 *   [P1] loadWorkflowState: handles both session.yaml and session.md formats.
 */

const fs   = require("node:fs");
const path = require("node:path");
const {
  hasCompleteRuntime,
  loadRuntimeSnapshot,
  projectWorkflowCompatState,
} = require("../runtime/runtime-snapshot.cjs");

// ─── Default policy ──────────────────────────────────────────────────────────

const DEFAULT_POLICY = {
  version: "2.0.0",

  pretoolBash: {
    bypassEnv:      "SY_BYPASS_PRETOOL_BASH",
    commitAllowEnv: "SY_ALLOW_GIT_COMMIT",
    pushAllowEnv:   "SY_ALLOW_GIT_PUSH",
    blockedCommands: [
      {
        name:   "force_push",
        // --force / --force-with-lease / -f flag immediately after push
        regex:  "\\bgit\\s+push\\b[\\s\\S]*(--force-with-lease|--force|\\s-f(\\s|$))",
        reason: "force push is prohibited — use --force-with-lease and get user confirmation",
        cites:  "sy-constraints/safety",
      },
      {
        name:   "git_reset_hard",
        regex:  "\\bgit\\s+reset\\b[\\s\\S]*\\s--hard(\\s|$)",
        reason: "git reset --hard is prohibited — use git stash to preserve work",
        cites:  "sy-constraints/safety",
      },
      {
        // Only block `git checkout -- <file>` (discard worktree changes).
        // Does NOT block `git checkout <branch> -- <file>` (restore from branch).
        name:   "git_checkout_discard",
        regex:  "\\bgit\\s+checkout\\s+--\\s+\\S",
        reason: "discard checkout is prohibited — stash or branch first",
        cites:  "sy-constraints/safety",
      },
      {
        name:   "rm_rf",
        regex:  "(^|[;&|\\n])\\s*rm\\s+-rf\\b",
        reason: "rm -rf is prohibited — list files first and confirm with user",
        cites:  "sy-constraints/safety",
      },
      {
        name:   "windows_del_force_recursive",
        regex:  "(^|[;&|])\\s*del\\s+\\/f[\\s\\S]*\\s\\/s\\b",
        reason: "Windows recursive forced delete is prohibited",
        cites:  "sy-constraints/safety",
      },
      {
        name:   "windows_rd_recursive_quiet",
        regex:  "(^|[;&|])\\s*rd\\s+\\/s\\s+\\/q\\b",
        reason: "Windows recursive directory delete is prohibited",
        cites:  "sy-constraints/safety",
      },
      {
        name:   "env_redirect",
        regex:  "(>>?|1>>?)\\s*\\.env\\b",
        reason: "writing to .env via shell redirect is prohibited — edit the file directly",
        cites:  "sy-constraints/appsec",
      },
    ],
  },

  pretoolWrite: {
    bypassEnv: "SY_BYPASS_PRETOOL_WRITE",
    // Files that must never be written by the agent directly
    protectedFiles: [
      { pattern: "\\.env$",              label: "root .env" },
      { pattern: "\\.env\\.",            label: ".env.* variant" },
      { pattern: "Cargo\\.lock$",        label: "Cargo.lock" },
      { pattern: "package-lock\\.json$", label: "package-lock.json" },
      { pattern: "pnpm-lock\\.yaml$",    label: "pnpm-lock.yaml" },
      { pattern: "yarn\\.lock$",         label: "yarn.lock" },
    ],
    tddGateEnabled: true,
  },

  secrets: {
    bypassEnv: "SY_BYPASS_SECRET_GUARD",
    placeholderPatterns: [
      // Word-bounded bare words — prevents false negatives like "AKIAIOSFODNN7EXAMPLE"
      // where "example" is a suffix, not a semantic placeholder marker.
      "\\byour[_-]?\\b",
      "\\bexample\\b",
      "\\bsample\\b",
      "\\bplaceholder\\b",
      "\\bchangeme\\b",
      "\\bdummy\\b",
      "\\bfake\\b",
      "\\btest[_-]?\\b",
      "^<.+>$",
      "^\\$\\{.+\\}$",
      "^\\$\\(.+\\)$",
    ],
    highConfidencePatterns: [
      { name: "private key",   regex: "-----BEGIN (?:RSA|EC|OPENSSH|DSA|PGP|PRIVATE) KEY-----" },
      { name: "aws access key",regex: "\\bAKIA[0-9A-Z]{16}\\b" },
      { name: "github token",  regex: "\\bgh[pousr]_[A-Za-z0-9]{20,}\\b" },
      { name: "slack token",   regex: "\\bxox[baprs]-[A-Za-z0-9-]{10,}\\b" },
      { name: "jwt",           regex: "\\beyJ[A-Za-z0-9_-]{10,}\\.[A-Za-z0-9._-]{10,}\\.[A-Za-z0-9._-]{10,}\\b" },
      { name: "stripe key",    regex: "\\bsk_(?:live|test)_[A-Za-z0-9]{24,}\\b" },
      { name: "sendgrid key",  regex: "\\bSG\\.[A-Za-z0-9_-]{22,}\\.[A-Za-z0-9_-]{22,}\\b" },
    ],
    assignmentKeyRegex:
      "(api[_-]?key|secret|token|password|passwd|api_secret)\\s*[:=]\\s*[\"'`]?([A-Za-z0-9_\\-+=./]{16,})[\"'`]?",
    allowedValueRegex:
      "process\\.env|import\\.meta\\.env|dotenv|os\\.environ|std::env::var|env!\\(",
  },

  stop: {
    bypassEnv:          "SY_BYPASS_STOP_GUARD",
    maxReportAgeHours:  6,
    reportRelativePath: ".ai/analysis/ai.report.json",
    lockStalenessMs:    30_000,
    // phases where we skip the stop gate entirely
    passPhases: ["done", "explore", "exploring", "benchmark", "benchmarking",
                 "free-ideation", "design", "designing", "brainstorm", "ideation", ""],
  },

  sessionStart: {
    enabled:              true,
    workflowSkill:        "sy-workflow",
    constraintsSkill:     "sy-constraints",
    maxChildSkillsPerTurn: 2,
  },

  promptRefresh: {
    bypassEnv:         "SY_BYPASS_PROMPT_REFRESH",
    // Inject constraint anchor only when session is active and prompt touches
    // one of these keywords.
    triggerKeywords: [
      "implement", "fix", "add", "write", "create", "build",
      "\u5b9e\u73b0", "\u4fee\u590d", "\u6dfb\u52a0", "\u7f16\u5199", "\u521b\u5efa", "\u6784\u5efa",
      "debug", "\u6d4b\u8bd5",
    ],
    // Phases where refresh is relevant
    activePhases: ["plan", "execute", "review"],
  },

  beforeToolSelection: {
    enabled: true,
    bypassEnv: "SY_BYPASS_TOOL_SELECTION",
    allowMode: "AUTO",
    denyMode: "NONE",
    restrictOnNonExecute: true,
    restrictOnParseError: true,
    blockOnApprovalPending: true,
    blockOnRestorePending: true,
    allowedFunctionNames: [
      "glob",
      "grep_search",
      "list_directory",
      "read_file",
      "read_many_files",
      "google_web_search",
      "web_fetch",
      "get_internal_docs",
      "activate_skill",
      "save_memory",
    ],
  },

  afterModel: {
    enabled: true,
    bypassEnv: "SY_BYPASS_AFTER_MODEL",
    maxRedactions: 5,
  },

  workflow: {
    sessionRelativePath: ".ai/workflow/session.yaml",
    donePhases:      ["done"],
    doneNextActions: ["none", "n/a", "na", "done", "completed", "idle", "wait new task",
                      "wait_new_task", "\u65e0", "\u5b8c\u6210"],
    maxActiveHours:  72,
  },
};

// ─── Policy loading (array-aware merge) ──────────────────────────────────────

/**
 * Deep-merge two policy objects.
 * Arrays are CONCATENATED (custom rules ADD to defaults, not replace them).
 * Scalar fields are overridden by the custom value.
 */
function deepMerge(base, custom) {
  if (!custom || typeof custom !== "object" || Array.isArray(custom)) return base;
  const result = { ...base };
  for (const [key, customVal] of Object.entries(custom)) {
    const baseVal = base[key];
    if (Array.isArray(baseVal) && Array.isArray(customVal)) {
      // Concatenate: project-specific entries extend the defaults
      result[key] = [...baseVal, ...customVal];
    } else if (
      baseVal && typeof baseVal === "object" && !Array.isArray(baseVal) &&
      customVal && typeof customVal === "object" && !Array.isArray(customVal)
    ) {
      result[key] = deepMerge(baseVal, customVal);
    } else {
      result[key] = customVal;
    }
  }
  return result;
}

function resolvePolicyPath(cwd) {
  const fromEnv = String(process.env.SY_HOOKS_POLICY || "").trim();
  if (fromEnv) {
    return path.isAbsolute(fromEnv) ? fromEnv : path.resolve(cwd, fromEnv);
  }
  return path.resolve(cwd, ".claude", "sy-hooks.policy.json");
}

function loadPolicy(cwd = process.cwd()) {
  const policyPath = resolvePolicyPath(cwd);
  try {
    if (!fs.existsSync(policyPath)) {
      return { policyPath, policy: DEFAULT_POLICY };
    }
    const custom = parseJsonSafe(fs.readFileSync(policyPath, "utf8"), {});
    return { policyPath, policy: deepMerge(DEFAULT_POLICY, custom) };
  } catch {
    return { policyPath, policy: DEFAULT_POLICY };
  }
}

// ─── I/O primitives ──────────────────────────────────────────────────────────

function readStdin() {
  return new Promise((resolve) => {
    let data = "";
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (c) => { data += c; });
    process.stdin.on("end",  () => resolve(data));
    process.stdin.on("error",() => resolve(""));
  });
}

function parseJsonSafe(text, fallback = {}) {
  try {
    const parsed = JSON.parse(text || "{}");
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return fallback;
    return parsed;
  } catch { return fallback; }
}

function asObject(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

/** Claude Code sends tool_input; some events send input. Normalize both. */
function resolveInput(payload) {
  const p = asObject(payload);
  return asObject(p.tool_input ?? p.input ?? p);
}

/** Extract the content being written for Write (content) or Edit (new_string). */
function resolveContent(payload) {
  const input = resolveInput(payload);
  return String(input.content ?? input.new_string ?? "");
}

/** Extract file path for Write|Edit. */
function resolveFilePath(payload) {
  const input = resolveInput(payload);
  return String(input.file_path ?? input.path ?? "");
}

function resolveCwd(payload) {
  return String(asObject(payload).cwd || process.cwd()) || process.cwd();
}

// ─── Response primitives ─────────────────────────────────────────────────────

/** Exit 0: allow the tool call. Optionally inject context for Claude. */
function allow(context) {
  if (context) {
    process.stdout.write(JSON.stringify({ context }));
  } else {
    process.stdout.write("{}");
  }
}

/**
 * Exit 2: BLOCK the tool call.
 * Claude receives stderr as context explaining why.
 */
function block(tag, reason, extraLines = []) {
  const lines = [`[${tag}] BLOCKED: ${reason}`, ...extraLines.map(l => `[${tag}] ${l}`)];
  process.stderr.write(lines.join("\n") + "\n");
  process.exit(2);
}

/** Non-blocking warning: writes to stderr without exit 2. */
function warn(tag, message, extraLines = []) {
  const lines = [`[${tag}] WARN: ${message}`, ...extraLines.map(l => `[${tag}] ${l}`)];
  process.stderr.write(lines.join("\n") + "\n");
}

// ─── Regex helpers ────────────────────────────────────────────────────────────

function compilePattern(pattern, flags = "i") {
  try { return new RegExp(String(pattern || ""), flags); }
  catch { return null; }
}

// ─── Secrets helpers ─────────────────────────────────────────────────────────

function isPlaceholder(value, patterns) {
  const v = String(value || "");
  return patterns.some((p) => {
    const re = compilePattern(p, "i");
    return re && re.test(v);
  });
}

/**
 * Scan content for secrets.
 * Returns { blocked: true, name, match } or { blocked: false }.
 */
function scanSecrets(content, secretPolicy) {
  const sp = asObject(secretPolicy);
  const placeholders = Array.isArray(sp.placeholderPatterns) ? sp.placeholderPatterns : [];
  const high = Array.isArray(sp.highConfidencePatterns) ? sp.highConfidencePatterns : [];

  for (const item of high) {
    const re = compilePattern(asObject(item).regex, "");
    if (!re) continue;
    const m = content.match(re);
    if (!m) continue;
    if (!isPlaceholder(m[0], placeholders)) {
      return { blocked: true, name: String(asObject(item).name || "unknown"), match: m[0].slice(0, 40) };
    }
  }

  const assignRe  = compilePattern(String(sp.assignmentKeyRegex || ""), "ig");
  const allowedRe = compilePattern(String(sp.allowedValueRegex  || ""), "i");
  if (assignRe) {
    for (const m of content.matchAll(assignRe)) {
      const keyName = String(m[1] || "");
      const value   = String(m[2] || "");
      if (!value || isPlaceholder(value, placeholders)) continue;
      if (allowedRe && allowedRe.test(value)) continue;
      return { blocked: true, name: `hardcoded credential key: ${keyName}`, match: value.slice(0, 40) };
    }
  }

  return { blocked: false };
}

// ─── Workflow state ───────────────────────────────────────────────────────────

function normalizeToken(value) {
  return String(value || "").trim().toLowerCase()
    .replace(/[`"'<>]/g, "").replace(/\s+/g, " ");
}

/**
 * Parse a flat key: value text file (session.yaml or legacy session.md).
 * Handles both `key: value` and `- key: value` formats.
 */
function parseSessionFields(content) {
  const fields = {};
  for (const line of String(content || "").split(/\r?\n/)) {
    const normalized = line.trim().replace(/^-\s*/, "");
    const m = normalized.match(/^([A-Za-z0-9_]+)\s*:\s*(.*)$/);
    if (!m) continue;
    const k = m[1].trim().toLowerCase();
    const v = m[2].trim();
    if (k) fields[k] = v;
  }
  return fields;
}

function getWorkflowSessionRelativePaths(workflowConfig = {}) {
  const configured = String(
    workflowConfig.sessionRelativePath || ".ai/workflow/session.yaml",
  ).trim() || ".ai/workflow/session.yaml";

  const candidates = [];
  const pushCandidate = (value) => {
    const candidate = String(value || "").trim();
    if (candidate && !candidates.includes(candidate)) candidates.push(candidate);
  };

  pushCandidate(configured);

  if (/session\.yaml$/i.test(configured)) {
    pushCandidate(configured.replace(/session\.yaml$/i, "session.md"));
  } else if (/session\.md$/i.test(configured)) {
    pushCandidate(configured.replace(/session\.md$/i, "session.yaml"));
  } else {
    pushCandidate(".ai/workflow/session.yaml");
    pushCandidate(".ai/workflow/session.md");
  }

  return candidates;
}

function computeWorkflowFlags(workflowConfig, phase, nextAction, updatedAt) {
  const donePhases = (Array.isArray(workflowConfig.donePhases) ? workflowConfig.donePhases : ["done"]).map(normalizeToken);
  const doneActions = (Array.isArray(workflowConfig.doneNextActions) ? workflowConfig.doneNextActions : ["done"]).map(normalizeToken);
  const maxMs = (Number(workflowConfig.maxActiveHours) || 72) * 3_600_000;
  const updatedMs = Date.parse(updatedAt);
  const isStale = Number.isFinite(updatedMs) && (Date.now() - updatedMs > maxMs);
  const isDone = donePhases.includes(phase) || doneActions.includes(nextAction);
  const hasPending = !isStale && !isDone && Boolean(phase || nextAction);
  return { isDone, isStale, hasPending };
}

function finalizeWorkflowState({
  exists,
  sessionPath,
  sessionRelPath,
  sessionRelPaths,
  fields,
  sourceModel,
  runtimeReady,
  parseError,
  snapshot,
  phaseId,
  activeNodeId,
  recommendedNext,
}, workflowConfig) {
  const normalizedFields = asObject(fields);
  const phase = normalizeToken(normalizedFields.current_phase ?? normalizedFields.phase ?? "");
  const nextAction = normalizeToken(normalizedFields.next_action ?? normalizedFields.next ?? "");
  const updatedAt = String(normalizedFields.updated_at ?? normalizedFields.updated ?? "").trim();
  const flags = computeWorkflowFlags(workflowConfig, phase, nextAction, updatedAt);
  return {
    exists,
    sessionPath,
    sessionRelPath,
    sessionRelPaths,
    phase,
    phaseId: String(phaseId || normalizedFields.phase_id || "").trim(),
    nextAction,
    updatedAt,
    runtimeReady,
    sourceModel,
    activeNodeId: String(activeNodeId || normalizedFields.active_node_id || "").trim(),
    recommendedNext: Array.isArray(recommendedNext) ? recommendedNext : [],
    fields: normalizedFields,
    snapshot: snapshot || null,
    parseError: Boolean(parseError),
    ...flags,
  };
}

function loadWorkflowState(cwd = process.cwd(), policyObj = DEFAULT_POLICY) {
  const wf = asObject(asObject(policyObj).workflow);
  const sessionRelPaths = getWorkflowSessionRelativePaths(wf);
  let sessionRelPath = sessionRelPaths[0];
  let sessionPath = path.resolve(cwd, sessionRelPath);

  const snapshot = loadRuntimeSnapshot(cwd);
  if (hasCompleteRuntime(snapshot)) {
    const compat = projectWorkflowCompatState(snapshot);
    return finalizeWorkflowState({
      exists: true,
      sessionPath: snapshot.paths.session,
      sessionRelPath: ".ai/workflow/session.yaml",
      sessionRelPaths,
      fields: compat.fields,
      sourceModel: "v4_runtime",
      runtimeReady: true,
      parseError: false,
      snapshot,
      phaseId: compat.phaseId,
      activeNodeId: compat.activeNodeId,
      recommendedNext: compat.recommendedNext,
    }, wf);
  }

  for (const candidate of sessionRelPaths) {
    const candidatePath = path.resolve(cwd, candidate);
    if (fs.existsSync(candidatePath)) {
      sessionRelPath = candidate;
      sessionPath = candidatePath;
      break;
    }
  }

  if (!fs.existsSync(sessionPath)) {
    return finalizeWorkflowState({
      exists: false,
      sessionPath,
      sessionRelPath,
      sessionRelPaths,
      fields: {},
      sourceModel: snapshot.sourceModel || "missing",
      runtimeReady: false,
      parseError: false,
      snapshot,
      phaseId: "",
      activeNodeId: "",
      recommendedNext: [],
    }, wf);
  }

  let raw = "";
  try { raw = fs.readFileSync(sessionPath, "utf8"); }
  catch {
    return finalizeWorkflowState({
      exists: true,
      sessionPath,
      sessionRelPath,
      sessionRelPaths,
      fields: {},
      sourceModel: snapshot.sourceModel || "legacy_flat",
      runtimeReady: false,
      parseError: true,
      snapshot,
      phaseId: "",
      activeNodeId: "",
      recommendedNext: [],
    }, wf);
  }

  const fields = parseSessionFields(raw);
  const looksLegacyFlat = ["current_phase", "next_action", "tdd_required", "red_verified", "last_completed_node"]
    .some((key) => Object.prototype.hasOwnProperty.call(fields, key));

  return finalizeWorkflowState({
    exists: true,
    sessionPath,
    sessionRelPath,
    sessionRelPaths,
    fields: looksLegacyFlat ? fields : {},
    sourceModel: looksLegacyFlat ? "legacy_flat" : (snapshot.sourceModel || "partial_runtime"),
    runtimeReady: false,
    parseError: !looksLegacyFlat,
    snapshot,
    phaseId: "",
    activeNodeId: String(fields.active_node_id || "").trim(),
    recommendedNext: [],
  }, wf);
}

// ─── Ledger helpers ───────────────────────────────────────────────────────────

/**
 * Count distinct verified nodes in ledger.md.
 * A node is verified when ledger has a line matching:  ### N{id} ✅
 * Returns { count, nodeIds: string[] }
 */
function countLedgerNodes(cwd) {
  const ledgerPath = path.resolve(cwd, ".ai/workflow/ledger.md");
  try {
    if (!fs.existsSync(ledgerPath)) return { count: 0, nodeIds: [] };
    const lines   = fs.readFileSync(ledgerPath, "utf8").split(/\r?\n/);
    const nodeIds = new Set();
    for (const line of lines) {
      // Match:  ### N3 ✅ …  or  ### N3 ✓ …
      const m = line.match(/^###\s+((?:P\d+-N\d+)|(?:N\d+)|(?:S\d+))\s+[✅✓]/u);
      if (m) nodeIds.add(m[1]);
    }
    return { count: nodeIds.size, nodeIds: [...nodeIds] };
  } catch { return { count: 0, nodeIds: [] }; }
}

// ─── Debug phase helpers ──────────────────────────────────────────────────────

/**
 * Load systematic-debugging phase state from workflow session debug_* fields.
 * Returns:
 *   { active: bool, phase: number|null, hypothesisCount: number, nodeId: string }
 *
 * Debug session fields written by systematic-debugging/SKILL.md:
 *   debug_active:        true | false
 *   debug_phase:         1 | 2 | 3 | 4 | 5   (Iron Law: writes only allowed at phase 5)
 *   debug_hypothesis_n:  count of hypotheses tried (max 3 before escalation)
 *   debug_node_id:       which plan node (or bug id) is being debugged
 */
function loadDebugState(sessionFields) {
  const f = asObject(sessionFields);
  const active       = String(f.debug_active  || "").toLowerCase() === "true";
  const phaseRaw     = String(f.debug_phase   || "").trim();
  const phase        = phaseRaw ? (parseInt(phaseRaw, 10) || null) : null;
  const hypothesisN  = parseInt(String(f.debug_hypothesis_n || "0"), 10) || 0;
  const nodeId       = String(f.debug_node_id || "").trim();
  return { active, phase, hypothesisCount: hypothesisN, nodeId };
}

// ─── Scope helpers ────────────────────────────────────────────────────────────

/**
 * Check whether filePath is "inside" the declared node target.
 * target is a relative path like "src/redirect.rs" or "tests/".
 * filePath is the absolute or relative path being written.
 *
 * Returns true when filePath is clearly within the target's directory or IS the target.
 * Returns null (unknown) when target is missing or ambiguous.
 */
function isInsideTarget(filePath, target, cwd) {
  if (!filePath || !target) return null;
  try {
    const absFile   = path.isAbsolute(filePath) ? filePath : path.resolve(cwd, filePath);
    const absTarget = path.isAbsolute(target)   ? target   : path.resolve(cwd, target);
    // If target looks like a directory (no extension or trailing slash)
    const targetIsDir = !path.extname(absTarget) || target.endsWith("/") || target.endsWith("\\");
    if (targetIsDir) {
      // file must be under the target directory
      return absFile.startsWith(absTarget + path.sep) || absFile === absTarget;
    }
    // target is a file — exact match or same directory
    return absFile === absTarget || path.dirname(absFile) === path.dirname(absTarget);
  } catch { return null; }
}

module.exports = {
  asObject,
  allow,
  block,
  warn,
  compilePattern,
  countLedgerNodes,
  deepMerge,
  isInsideTarget,
  isPlaceholder,
  loadDebugState,
  loadPolicy,
  loadWorkflowState,
  normalizeToken,
  parseJsonSafe,
  parseSessionFields,
  readStdin,
  resolveCwd,
  resolveContent,
  resolveFilePath,
  resolveInput,
  scanSecrets,
  DEFAULT_POLICY,
};
