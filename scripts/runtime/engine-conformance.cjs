"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const { writeClaudeCodeArtifacts } = require("../adapters/claude-code.cjs");
const { writeCodexArtifacts } = require("../adapters/codex.cjs");
const { writeGeminiArtifacts } = require("../adapters/gemini-cli.cjs");
const { compactContext } = require("./context-manager.cjs");
const { recoverInterruptedRun } = require("./checkpoints.cjs");
const { buildShortApprovalRequest } = require("./human-output.cjs");
const { appendEvent } = require("./journal.cjs");
const { listCapsules, writeSession, writeSprintStatus, writeTaskGraph } = require("./store.cjs");
const { loadWorkflowSpecs } = require("./workflow-specs.cjs");

const ENGINES = ["claude_code", "codex", "gemini_cli"];
const DEFAULT_REVIEW_CHAIN = ["author", "spec_reviewer", "quality_reviewer"];
const ENGINE_WRITERS = {
  claude_code: writeClaudeCodeArtifacts,
  codex: writeCodexArtifacts,
  gemini_cli: writeGeminiArtifacts,
};

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function makeTempRoot(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function readText(filePath) {
  return fs.readFileSync(filePath, "utf8");
}

function readJsonIfExists(filePath) {
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(readText(filePath));
}

function runNodeHook(projectRoot, scriptPath, payload) {
  const result = spawnSync(process.execPath, [scriptPath], {
    cwd: projectRoot,
    input: JSON.stringify(payload),
    encoding: "utf8",
  });
  return {
    code: Number.isInteger(result.status) ? result.status : 1,
    stdout: String(result.stdout || "").trim(),
    stderr: String(result.stderr || "").trim(),
  };
}

function loadEngineArtifact(rootDir, engine) {
  const writer = ENGINE_WRITERS[engine];
  if (typeof writer !== "function") {
    throw new Error(`Unsupported engine: ${engine}`);
  }

  const outputRootDir = makeTempRoot(`sy-engine-${engine}-`);
  const rendered = writer({ rootDir, outputRootDir });
  const instructionFile = rendered.bundle.render_targets.instruction_file;
  const instructionPath = path.join(outputRootDir, instructionFile);
  const configFiles = Array.isArray(rendered.bundle.render_targets.config_files)
    ? rendered.bundle.render_targets.config_files
    : [];

  const configContents = {};
  const configJson = {};
  for (const configFile of configFiles) {
    const absolutePath = path.join(outputRootDir, configFile);
    configContents[configFile] = readText(absolutePath);
    if (/\.json$/i.test(configFile)) {
      configJson[configFile] = readJsonIfExists(absolutePath);
    }
  }

  return {
    engine,
    rootDir: normalizePath(rootDir),
    outputRootDir: normalizePath(outputRootDir),
    bundle: rendered.bundle,
    writtenFiles: Array.isArray(rendered.written_files) ? rendered.written_files.map(normalizePath) : [],
    instructionFile,
    instructionText: readText(instructionPath),
    configFiles,
    configContents,
    configJson,
    settings: rendered.settings || null,
  };
}

function loadAllEngineArtifacts(rootDir) {
  const absoluteRoot = path.resolve(rootDir);
  return ENGINES.reduce((accumulator, engine) => {
    accumulator[engine] = loadEngineArtifact(absoluteRoot, engine);
    return accumulator;
  }, {});
}

function hasRequiredHook(artifact, eventName) {
  const hooks = artifact?.bundle?.hook_contract?.hooks;
  if (!Array.isArray(hooks)) {
    return false;
  }
  return hooks.some((hook) => hook.event === eventName && hook.required === true);
}

function summarizeArtifact(artifact) {
  const bundle = artifact.bundle;
  const instructionNotice = /Generated artifact/i.test(artifact.instructionText)
    && /source of truth/i.test(artifact.instructionText);
  const humanBlockerSurface = /approval requests MUST use runtime-approved zh-CN short actionable copy/i.test(artifact.instructionText)
    && /manual restore blockers MUST use runtime-approved zh-CN short actionable copy/i.test(artifact.instructionText)
    && /`recommended_next` and `restore_reason` MUST come from runtime state/i.test(artifact.instructionText)
    && /runtime approval request in zh-CN short actionable copy/i.test(artifact.instructionText)
    && /runtime restore request in zh-CN short actionable copy/i.test(artifact.instructionText);
  const expectedReviewChain = JSON.stringify(DEFAULT_REVIEW_CHAIN);
  const actualReviewChain = JSON.stringify(Array.isArray(bundle.review_chain) ? bundle.review_chain : []);

  return {
    engine: artifact.engine,
    language_policy:
      bundle?.language_policy?.agent_rule_language === "en"
      && bundle?.language_policy?.human_output_language === "zh-CN"
      && bundle?.language_policy?.approval_request_style === "short_explicit_action_oriented",
    instruction_notice: instructionNotice,
    render_targets:
      artifact.writtenFiles.includes(bundle.render_targets.instruction_file)
      && bundle.render_targets.config_files.every((filePath) => artifact.writtenFiles.includes(filePath)),
    review_chain: actualReviewChain === expectedReviewChain,
    dangerous_command_surface:
      hasRequiredHook(artifact, "PreToolUse:Bash")
      && typeof bundle?.engine_contract?.approval_surface === "string"
      && bundle.engine_contract.approval_surface.length > 0,
    human_blocker_surface: humanBlockerSurface,
    tdd_guard_surface: hasRequiredHook(artifact, "PreToolUse:Write|Edit"),
    resume_surface:
      hasRequiredHook(artifact, "Stop")
      && (artifact.engine !== "gemini_cli"
        || artifact.settings?.general?.checkpointing?.enabled === true
        || artifact.configJson[".gemini/settings.json"]?.general?.checkpointing?.enabled === true),
  };
}

function assertCoreSurfaceAlignment(artifacts, options = {}) {
  const requiredChecks = Array.isArray(options.requiredChecks) && options.requiredChecks.length > 0
    ? options.requiredChecks
    : [
        "language_policy",
        "instruction_notice",
        "render_targets",
        "review_chain",
        "dangerous_command_surface",
        "human_blocker_surface",
        "tdd_guard_surface",
        "resume_surface",
      ];

  const summaries = Object.values(artifacts).map((artifact) => summarizeArtifact(artifact));
  for (const summary of summaries) {
    for (const checkName of requiredChecks) {
      if (summary[checkName] !== true) {
        throw new Error(`ENGINE_CONFORMANCE_SURFACE_FAIL engine=${summary.engine} check=${checkName}`);
      }
    }
  }
  return summaries;
}

function buildCanonicalApprovalRequest(rootDir) {
  const specs = loadWorkflowSpecs(rootDir);
  const criticalPolicyFile = specs?.approvalMatrix?.file_classes?.critical_policy_file || {};
  return {
    actionLabel: "写入文件",
    targetRef: "workflow/router.spec.yaml (critical_policy_file)",
    riskClass: criticalPolicyFile.risk_class || "high",
    approvalMode: criticalPolicyFile.approval_mode || "manual_required",
    grantScopes: criticalPolicyFile.allowed_grant_scopes || ["once"],
  };
}

function renderCanonicalApprovalCopy(rootDir, artifact) {
  const languagePolicy = artifact.bundle?.language_policy || {};
  if (languagePolicy.agent_rule_language !== "en") {
    throw new Error(`ENGINE_APPROVAL_COPY_DRIFT engine=${artifact.engine} field=agent_rule_language actual=${JSON.stringify(languagePolicy.agent_rule_language)}`);
  }
  if (languagePolicy.human_output_language !== "zh-CN") {
    throw new Error(`ENGINE_APPROVAL_COPY_DRIFT engine=${artifact.engine} field=human_output_language actual=${JSON.stringify(languagePolicy.human_output_language)}`);
  }
  if (languagePolicy.approval_request_style !== "short_explicit_action_oriented") {
    throw new Error(`ENGINE_APPROVAL_COPY_DRIFT engine=${artifact.engine} field=approval_request_style actual=${JSON.stringify(languagePolicy.approval_request_style)}`);
  }

  return buildShortApprovalRequest(buildCanonicalApprovalRequest(rootDir)).join("\n");
}

function assertApprovalCopyAligned(artifacts) {
  const entries = Object.values(artifacts);
  assert(entries.length === ENGINES.length, `expected ${ENGINES.length} engine artifacts but got ${entries.length}`);

  let baseline = null;
  for (const artifact of entries) {
    const rendered = renderCanonicalApprovalCopy(artifact.rootDir, artifact);
    if (!baseline) {
      baseline = { engine: artifact.engine, rendered };
      continue;
    }
    if (rendered !== baseline.rendered) {
      throw new Error(`ENGINE_APPROVAL_COPY_DRIFT engine=${artifact.engine} baseline=${baseline.engine}`);
    }
  }
}

function nowIso() {
  return new Date().toISOString();
}

function buildConformanceSession(workspace) {
  return {
    schema: 4,
    run_id: "wf-20260309-003",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-p6-conformance", title: "Engine conformance", mode: "feature" },
    phase: { current: "P6", status: "in_progress" },
    node: { active_id: "P6-N1", state: "red_pending", owner_persona: "author" },
    loop_budget: {
      max_nodes: 10,
      max_failures: 2,
      max_pending_approvals: 1,
      consumed_nodes: 0,
      consumed_failures: 0,
    },
    context_budget: {
      strategy: "hybrid",
      capsule_refresh_threshold: 4,
      summary_required_after_turns: 8,
    },
    workspace: { root: normalizePath(workspace), sandbox_mode: "workspace_write" },
    approvals: {
      pending: false,
      pending_count: 0,
      last_grant_scope: "none",
      last_approval_mode: "none",
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: null,
      restore_pending: false,
      restore_reason: null,
    },
    timestamps: {
      created_at: nowIso(),
      updated_at: nowIso(),
    },
  };
}

function buildConformanceTaskGraph() {
  return {
    schema: 4,
    graph_id: "graph-p6-conformance",
    phases: [
      {
        id: "P6",
        title: "Engine conformance",
        status: "in_progress",
        depends_on: ["P5"],
        entry_condition: ["P5 completed"],
        exit_gate: {
          cmd: "node tests/e2e/run-engine-conformance.cjs --all",
          pass_signal: "ENGINE_CONFORMANCE_PASS",
          coverage_min: "80%",
        },
        rollback_boundary: {
          revert_nodes: ["P6-N1"],
          restore_point: "P5 stable",
        },
      },
    ],
    nodes: [
      {
        id: "P6-N1",
        phase_id: "P6",
        title: "Engine conformance matrix",
        target: "tests/e2e/run-engine-conformance.cjs",
        action: "Verify approval copy, dangerous command guard, TDD gate, resume frontier, and adapter outputs",
        why: "Prevent engine-specific workflow drift",
        depends_on: [],
        verify: {
          cmd: "node tests/e2e/run-engine-conformance.cjs --all",
          pass_signal: "ENGINE_CONFORMANCE_PASS",
        },
        risk_level: "critical",
        tdd_required: true,
        status: "in_progress",
        tdd_state: "red_pending",
        owner_persona: "author",
        review_state: { spec_review: "pending", quality_review: "pending" },
        evidence_refs: [],
        output_refs: [],
        approval_ref: null,
        capability: "code_edit",
        priority: "critical",
        parallel_group: null,
        condition: null,
        retry_policy: null,
        timeout_policy: null,
        test_contract: {
          layer: "e2e",
          coverage_mode: "full",
          coverage_profile: "critical",
          mock_policy: "boundary_only",
          acceptance_criteria_refs: ["AC-P6-CONFORMANCE-1"],
          red_cmd: "node tests/e2e/run-engine-conformance.cjs --case approval-copy-drift",
          green_cmd: "node tests/e2e/run-engine-conformance.cjs --all",
          red_expectation: {
            allowed_failure_kinds: ["contract_mismatch", "behavior_result_mismatch"],
            rejected_failure_kinds: ["syntax_error", "environment_error", "permission_error"],
            allowed_exit_codes: [1],
            stderr_pattern: null,
            error_type: null,
          },
          behavior_gate: {
            ac_traceability_required: true,
            boundary_conditions_required: true,
          },
        },
      },
    ],
  };
}

function buildConformanceSprintStatus() {
  return {
    schema: 4,
    active_phase: "P6",
    node_summary: [
      { id: "P6-N1", status: "in_progress", tdd_state: "red_pending" },
    ],
    recommended_next: [
      {
        type: "resume_node",
        target: "P6-N1",
        params: { mode: "verify" },
        reason: "continue engine conformance verification",
        blocking_on: [],
        priority: "now",
      },
    ],
  };
}

function seedConformanceRuntime(workspace) {
  writeSession(workspace, buildConformanceSession(workspace));
  writeTaskGraph(workspace, buildConformanceTaskGraph());
  writeSprintStatus(workspace, buildConformanceSprintStatus());
}

function assertDangerousCommandGuard(projectRoot) {
  const result = runNodeHook(projectRoot, path.join(projectRoot, "scripts", "hooks", "sy-pretool-bash.cjs"), {
    cwd: projectRoot,
    tool_name: "Bash",
    tool_input: { command: "git push --force origin main" },
  });

  if (result.code !== 2) {
    throw new Error(`ENGINE_DANGEROUS_COMMAND_GUARD_FAIL exit=${result.code} stderr=${JSON.stringify(result.stderr)}`);
  }
  return result;
}

function assertTddWriteGuard(projectRoot) {
  const workspace = makeTempRoot("sy-engine-tdd-");
  seedConformanceRuntime(workspace);
  appendEvent(workspace, {
    runId: "wf-20260309-003",
    event: "node_started",
    phase: "P6",
    nodeId: "P6-N1",
    actor: "author",
    payload: { step: "start" },
  });

  const result = runNodeHook(projectRoot, path.join(projectRoot, "scripts", "hooks", "sy-pretool-write.cjs"), {
    cwd: workspace,
    tool_name: "Write",
    tool_input: {
      file_path: "src/runtime/conformance.ts",
      content: "export const conformance = true;\n",
    },
  });

  if (result.code !== 2) {
    throw new Error(`ENGINE_TDD_GUARD_FAIL exit=${result.code} stderr=${JSON.stringify(result.stderr)}`);
  }
  const output = `${result.stderr}\n${result.stdout}`;
  if (!/TDD|RED/i.test(output)) {
    throw new Error(`ENGINE_TDD_GUARD_FAIL missing_red_message=${JSON.stringify(output)}`);
  }
  return result;
}

function assertResumeFrontierAlignment(projectRoot) {
  const workspace = makeTempRoot("sy-engine-frontier-");
  seedConformanceRuntime(workspace);
  appendEvent(workspace, {
    runId: "wf-20260309-003",
    event: "node_started",
    phase: "P6",
    nodeId: "P6-N1",
    actor: "author",
    payload: { step: "start" },
  });

  const recovery = recoverInterruptedRun(workspace, { actor: "runtime" });
  if (recovery.recovery_required !== true) {
    throw new Error(`ENGINE_RESUME_FRONTIER_FAIL recovery_required=${JSON.stringify(recovery.recovery_required)}`);
  }
  if (!Array.isArray(recovery.recommended_next) || recovery.recommended_next[0]?.target !== "P6-N1") {
    throw new Error(`ENGINE_RESUME_FRONTIER_FAIL recovery_next=${JSON.stringify(recovery.recommended_next)}`);
  }

  const compacted = compactContext(workspace, {
    contextUtilization: 0.84,
    turnsSinceSummary: 9,
    turnsSinceCapsule: 4,
  });
  if (compacted.compacted !== true) {
    throw new Error(`ENGINE_RESUME_FRONTIER_FAIL compacted=${JSON.stringify(compacted.compacted)}`);
  }
  if (compacted.resume_frontier?.recommended_next?.[0]?.target !== "P6-N1") {
    throw new Error(`ENGINE_RESUME_FRONTIER_FAIL compact_frontier=${JSON.stringify(compacted.resume_frontier)}`);
  }
  if (listCapsules(workspace).length < 1) {
    throw new Error("ENGINE_RESUME_FRONTIER_FAIL capsules_missing");
  }

  return { recovery, compacted };
}

module.exports = {
  assertApprovalCopyAligned,
  assertCoreSurfaceAlignment,
  assertDangerousCommandGuard,
  assertResumeFrontierAlignment,
  assertTddWriteGuard,
  loadAllEngineArtifacts,
  loadEngineArtifact,
  summarizeArtifact,
};
