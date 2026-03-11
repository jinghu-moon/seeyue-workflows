#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { compileAdapterBundle } = require("./compile-adapter.cjs");
const {
  attachGeneratedMetadata,
  buildGeneratedMetadata,
  wrapGeneratedSection,
} = require("./adapter-utils.cjs");
const { loadWorkflowSpecs } = require("../runtime/workflow-specs.cjs");
const { buildSkillsManifest } = require("../runtime/skills-manifest.cjs");

const SOURCE_OF_TRUTH_FILES = [
  "workflow/runtime.schema.yaml",
  "workflow/router.spec.yaml",
  "workflow/policy.spec.yaml",
  "workflow/capabilities.yaml",
  "workflow/persona-bindings.yaml",
  "workflow/file-classes.yaml",
  "workflow/approval-matrix.yaml",
  "workflow/hooks.spec.yaml",
  "docs/architecture-v4.md",
];

const EXPECTED_HIERARCHY_MODE = "global_workspace_and_jit_context";

const GEMINI_HOOK_GROUPS = [
  {
    event: "SessionStart",
    description: "SY bootstrap: inject first-turn workflow + constraint routing context",
    hooks: [
      { name: "sy-session-start", delegate: "scripts/hooks/sy-session-start.cjs", mode: "session-start", timeout: 8000 },
    ],
  },
  {
    event: "BeforeAgent",
    description: "SY refresh: re-anchor long-session routing context before planning",
    hooks: [
      { name: "sy-prompt-refresh", delegate: "scripts/hooks/sy-prompt-refresh.cjs", mode: "before-agent", timeout: 6000 },
    ],
  },
  {
    event: "BeforeToolSelection",
    description: "SY filter: restrict tool selection scope when required",
    hooks: [
      { name: "sy-before-tool-selection", delegate: "scripts/hooks/sy-before-tool-selection.cjs", mode: "before-tool-selection", timeout: 6000 },
    ],
  },
  {
    event: "BeforeTool",
    matcher: "run_shell_command",
    sequential: true,
    description: "SY guard: command class approval, destructive guard, and loop budgets",
    hooks: [
      { name: "sy-pretool-bash", delegate: "scripts/hooks/sy-pretool-bash.cjs", mode: "before-tool", timeout: 10000 },
      { name: "sy-pretool-bash-budget", delegate: "scripts/hooks/sy-pretool-bash-budget.cjs", mode: "before-tool", timeout: 8000 },
    ],
  },
  {
    event: "BeforeTool",
    matcher: "write_file|replace",
    sequential: true,
    description: "SY guard: TDD, secret, protected-file, and session-integrity checks",
    hooks: [
      { name: "sy-pretool-write", delegate: "scripts/hooks/sy-pretool-write.cjs", mode: "before-tool", timeout: 10000 },
      { name: "sy-pretool-write-session", delegate: "scripts/hooks/sy-pretool-write-session.cjs", mode: "before-tool", timeout: 8000 },
    ],
  },
  {
    event: "AfterTool",
    matcher: "write_file|replace",
    sequential: true,
    description: "SY post-write: audit evidence and scope drift capture",
    hooks: [
      { name: "sy-posttool-write", delegate: "scripts/hooks/sy-posttool-write.cjs", mode: "after-tool", timeout: 8000 },
    ],
  },
  {
    event: "AfterTool",
    matcher: "run_shell_command",
    sequential: true,
    description: "SY post-shell: verification + TDD red/green evidence capture",
    hooks: [
      { name: "sy-posttool-bash-verify", delegate: "scripts/hooks/sy-posttool-bash-verify.cjs", mode: "after-tool", timeout: 8000 },
    ],
  },
  {
    event: "AfterModel",
    description: "SY post-model: redaction/logging boundary for Gemini streaming",
    hooks: [
      { name: "sy-after-model", delegate: "scripts/hooks/sy-after-model.cjs", mode: "after-model", timeout: 8000 },
    ],
  },
  {
    event: "AfterAgent",
    description: "SY completion gate: checkpoint and resume-frontier enforcement",
    hooks: [
      { name: "sy-stop", delegate: "scripts/hooks/sy-stop.cjs", mode: "after-agent", timeout: 10000 },
    ],
  },
];

const GEMINI_POLICY_FILE = ".gemini/policies/seeyue-workflows.toml";
const GEMINI_SAFE_TOOLS = [
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
];

const POLICY_PRIORITY = {
  allow: 100,
  ask_user: 700,
  deny: 950,
};

const RISK_PRIORITY = {
  low: 600,
  medium: 700,
  high: 800,
  critical: 900,
};

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function resolveRootDir(options = {}, bundle = null) {
  if (options.rootDir) {
    return path.resolve(options.rootDir);
  }
  if (bundle && bundle.generated_from_root) {
    return path.resolve(bundle.generated_from_root);
  }
  return path.resolve(__dirname, "..", "..");
}

function getBundle(options = {}) {
  if (options.bundle) {
    return options.bundle;
  }
  return compileAdapterBundle({ rootDir: options.rootDir, engine: "gemini_cli" });
}

function assertGeminiBundle(bundle) {
  if (!bundle || bundle.engine !== "gemini_cli") {
    throw new Error(`GEMINI_ADAPTER_ENGINE_MISMATCH expected=gemini_cli actual=${JSON.stringify(bundle && bundle.engine)}`);
  }
}

function validateHierarchyContract(bundle) {
  assertGeminiBundle(bundle);
  if (bundle.engine_contract.hierarchy_mode !== EXPECTED_HIERARCHY_MODE) {
    throw new Error(
      `GEMINI_ADAPTER_HIERARCHY_LOSS expected=${EXPECTED_HIERARCHY_MODE} actual=${JSON.stringify(bundle.engine_contract.hierarchy_mode)}`,
    );
  }
  if (bundle.render_targets.instruction_file !== "GEMINI.md") {
    throw new Error(
      `GEMINI_ADAPTER_HIERARCHY_LOSS instruction_file=${JSON.stringify(bundle.render_targets.instruction_file)}`,
    );
  }
}

function renderBridgeCommand(hook) {
  return `node scripts/hooks/gemini-hook-bridge.cjs --mode ${hook.mode} --delegate ${normalizePath(hook.delegate)}`;
}

function escapeTomlString(value) {
  return String(value || "").replace(/\\/g, "\\\\").replace(/"/g, '\\"');
}

function formatTomlValue(value) {
  if (Array.isArray(value)) {
    return `[${value.map(formatTomlValue).join(", ")}]`;
  }
  if (value && typeof value === "object") {
    const entries = Object.entries(value).map(([key, entry]) => `${key} = ${formatTomlValue(entry)}`);
    return `{ ${entries.join(", ")} }`;
  }
  if (typeof value === "string") {
    return `"${escapeTomlString(value)}"`;
  }
  return String(value);
}

function renderPolicyRules(rules) {
  const fieldOrder = [
    "toolName",
    "mcpName",
    "commandPrefix",
    "commandRegex",
    "argsPattern",
    "toolAnnotations",
    "decision",
    "priority",
    "modes",
    "allow_redirection",
    "deny_message",
  ];
  const lines = [
    "# seeyue-workflows Gemini CLI policy",
    "# Generated from workflow specs; do not edit manually.",
    "",
  ];

  for (const rule of rules) {
    lines.push("[[rule]]");
    for (const field of fieldOrder) {
      if (rule[field] === undefined || rule[field] === null) {
        continue;
      }
      lines.push(`${field} = ${formatTomlValue(rule[field])}`);
    }
    lines.push("");
  }

  return `${lines.join("\n").trimEnd()}\n`;
}

function globToRegex(pattern) {
  const normalized = String(pattern || "").replace(/\\/g, "/");
  if (!normalized) {
    return null;
  }
  let regex = normalized.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  regex = regex.replace(/\\\*\\\*/g, ".*");
  regex = regex.replace(/\\\*/g, "[^/]*");
  regex = regex.replace(/\\\?/g, "[^/]");
  regex = regex.replace(/\//g, "[\\\\/]");
  return regex;
}

function stripCommandPrefixGuard(pattern) {
  const raw = String(pattern || "");
  const guard = "(^|[;&|\\n])\\s*";
  const guardAlt = "(^|[;&|\\\\n])\\\\s*";
  if (raw.startsWith(guard)) {
    return raw.slice(guard.length);
  }
  if (raw.startsWith(guardAlt)) {
    return raw.slice(guardAlt.length);
  }
  return raw;
}

function mergePatterns(patterns) {
  const cleaned = patterns.map((entry) => stripCommandPrefixGuard(entry)).filter(Boolean);
  if (cleaned.length === 0) {
    return null;
  }
  if (cleaned.length === 1) {
    return cleaned[0];
  }
  return `(?:${cleaned.join("|")})`;
}

function resolveGeminiModes(approvalMode, modeMapping) {
  const modes = new Set();
  if (approvalMode === "manual_required") {
    modes.add("default");
    modes.add("autoEdit");
  }
  if (approvalMode === "never_auto") {
    modes.add("default");
    modes.add("autoEdit");
    modes.add("yolo");
  }
  if (approvalMode === "notify_only") {
    modes.add("default");
  }
  const mapped = modeMapping?.[approvalMode];
  if (mapped && mapped !== "plan") {
    modes.add(mapped);
  }
  return modes.size > 0 ? Array.from(modes) : undefined;
}

function buildGeminiPolicyRules(specs) {
  const approvalMatrix = specs.approvalMatrix || {};
  const fileClassRegistry = specs.fileClasses || {};
  const hookSpec = specs.hookSpec || {};
  const policySpec = specs.policySpec || {};
  const commandClasses = approvalMatrix.command_classes || {};
  const fileClasses = approvalMatrix.file_classes || {};
  const commandPatterns = hookSpec.command_classification?.classes || {};
  const modeMapping = policySpec.approval_policy?.engine_mode_mapping?.gemini_cli || {};

  const rules = [];

  rules.push({
    toolName: GEMINI_SAFE_TOOLS,
    decision: "allow",
    priority: POLICY_PRIORITY.allow,
    modes: ["default", "autoEdit", "plan"],
  });

  for (const [classId, classSpec] of Object.entries(commandClasses)) {
    if (!classSpec || classSpec.approval_required !== true) {
      continue;
    }
    const approvalMode = String(classSpec.approval_mode || "manual_required");
    const modes = resolveGeminiModes(approvalMode, modeMapping);
    const patterns = Array.isArray(commandPatterns[classId]?.patterns)
      ? commandPatterns[classId].patterns.map((entry) => entry.regex)
      : [];
    const commandRegex = mergePatterns(patterns.filter(Boolean));
    if (!commandRegex) {
      continue;
    }
    rules.push({
      toolName: "run_shell_command",
      commandRegex,
      decision: "ask_user",
      priority: RISK_PRIORITY[classSpec.risk_class] || POLICY_PRIORITY.ask_user,
      modes,
    });
  }

  for (const [classId, classSpec] of Object.entries(fileClasses)) {
    if (!classSpec || classSpec.approval_required !== true) {
      continue;
    }
    const approvalMode = String(classSpec.approval_mode || "manual_required");
    const registryPatterns = fileClassRegistry.classes?.[classId]?.patterns;
    const patterns = Array.isArray(registryPatterns)
      ? registryPatterns.map(globToRegex).filter(Boolean)
      : [];
    const merged = mergePatterns(patterns);
    if (!merged) {
      continue;
    }
    const decision = approvalMode === "never_auto" ? "deny" : "ask_user";
    const modes = decision === "deny" ? undefined : resolveGeminiModes(approvalMode, modeMapping);
    rules.push({
      toolName: ["write_file", "replace"],
      argsPattern: `"file_path":"${merged}"`,
      decision,
      priority: decision === "deny" ? POLICY_PRIORITY.deny : RISK_PRIORITY[classSpec.risk_class] || POLICY_PRIORITY.ask_user,
      modes,
      deny_message: decision === "deny" ? `该文件类型禁止直接写入：${classId}` : undefined,
    });
  }

  return rules;
}

function buildGeminiPolicyToml(bundle, options = {}) {
  const rootDir = resolveRootDir(options, bundle);
  const specs = loadWorkflowSpecs(rootDir);
  const rules = buildGeminiPolicyRules(specs);
  return renderPolicyRules(rules);
}

function ensureGeminiHookFilesPresent(rootDir) {
  const requiredFiles = new Set(["scripts/hooks/gemini-hook-bridge.cjs"]);
  for (const group of GEMINI_HOOK_GROUPS) {
    for (const hook of group.hooks) {
      requiredFiles.add(hook.delegate);
    }
  }

  for (const relativePath of requiredFiles) {
    const absolutePath = path.join(rootDir, relativePath);
    if (!fs.existsSync(absolutePath)) {
      throw new Error(`GEMINI_ADAPTER_MISSING_REQUIRED_HOOK script=${normalizePath(relativePath)}`);
    }
  }
}

function buildGeminiSettings(bundle, options = {}) {
  validateHierarchyContract(bundle);
  const rootDir = resolveRootDir(options, bundle);
  ensureGeminiHookFilesPresent(rootDir);

  return {
    context: {
      fileName: ["GEMINI.md"],
    },
    general: {
      defaultApprovalMode: "default",
      checkpointing: {
        enabled: true,
      },
    },
    skills: {
      enabled: true,
      disabled: [],
    },
    hooks: GEMINI_HOOK_GROUPS.reduce((accumulator, group) => {
      if (!accumulator[group.event]) {
        accumulator[group.event] = [];
      }

      const renderedGroup = {
        hooks: group.hooks.map((hook) => ({
          name: hook.name,
          type: "command",
          command: renderBridgeCommand(hook),
          timeout: hook.timeout,
          description: group.description,
        })),
      };

      if (group.matcher) {
        renderedGroup.matcher = group.matcher;
      }
      if (group.sequential) {
        renderedGroup.sequential = true;
      }

      accumulator[group.event].push(renderedGroup);
      return accumulator;
    }, {}),
  };
}

function buildGeminiInstructions(bundle) {
  validateHierarchyContract(bundle);
  const reviewChain = bundle.review_chain.map((entry) => `\`${entry}\``).join(" -> ");
  const sourceOfTruth = SOURCE_OF_TRUTH_FILES.map((entry) => `- \`${entry}\``);

  const lines = [
    "# GEMINI.md",
    "",
    "> Generated artifact for `gemini_cli`. Do not edit manually.",
    "> Vendor files are deployment artifacts. `workflow/*.yaml` remains the machine source of truth.",
    "",
    "## Context Hierarchy",
    "",
    "- Preserve Gemini CLI hierarchy: global `~/.gemini/GEMINI.md`, workspace `GEMINI.md`, and just-in-time directory `GEMINI.md` files.",
    "- Treat this root `GEMINI.md` as the workspace policy layer, not as a free-form scratchpad.",
    "- Keep directory-local `GEMINI.md` files narrow and component-scoped so JIT loading stays precise.",
    "- Project settings in `.gemini/settings.json` override user and system settings, then extension hooks are merged afterward.",
    "",
    "## Language Policy",
    "",
    "- Write machine-facing rules, contracts, plans, and skill logic in English.",
    "- Write human-facing approvals, blockers, and status updates in concise zh-CN.",
    "- Keep approval text short, explicit, and action-oriented.",
    "- Human-facing approval requests MUST use runtime-approved zh-CN short actionable copy.",
    "- Human-facing manual restore blockers MUST use runtime-approved zh-CN short actionable copy.",
    "",
    "## Source Of Truth",
    "",
    ...sourceOfTruth,
    "- `.gemini/settings.json`",
    "",
    "## Routing Summary",
    "",
    "- Execution is state-first and blocker-first.",
    "- V4 Phase 1 uses a single active phase and a single active node.",
    `- Default review chain: ${reviewChain}.`,
    "- `recommended_next` and `restore_reason` MUST come from runtime state, not free-form chat reasoning.",
    "",
    "## Hooks And Approval",
    "",
    "- Use native Gemini hooks from `.gemini/settings.json` as hard guards at `SessionStart`, `BeforeAgent`, `BeforeTool`, `AfterTool`, and `AfterAgent`.",
    "- Keep `general.defaultApprovalMode = \"default\"` as the safe baseline; do not rely on `auto_edit` or `yolo` for normal workflow execution.",
    "- Project hooks are fingerprinted by Gemini CLI; treat hook changes as trusted-project review boundaries.",
    "- If runtime enters `approval_pending`, surface the runtime approval request in zh-CN short actionable copy and wait.",
    "",
    "## Planning Mode",
    "",
    "- During planning, design, or review-only phases, stay read-only unless the runtime explicitly enters an execution node.",
    "- Do not write files, mutate schema, or run risky commands before router and policy gates allow execution.",
    "- Use the workflow runtime state under `.ai/workflow/` as the execution boundary contract.",
    "",
    "## Recovery And Checkpointing",
    "",
    "- Keep Gemini checkpointing enabled so interrupted write operations can recover through `/restore`.",
    "- If runtime enters `restore_pending`, resolve recovery before any new write or command.",
    "- Resume from checkpointed state before proposing new writes after interruption.",
    "- If manual intervention is required, surface the runtime restore request in zh-CN short actionable copy and stop.",
    "- Treat runtime recovery state as authoritative over chat recap when the two diverge.",
    "",
    "## Skills And Isolation",
    "",
    "- Use skills progressively and load only the minimum files required for the active task.",
    "- Keep reviewer personas isolated from author context.",
    "- Do not bypass router, policy, or recovery boundaries through ad-hoc role switching.",
    "",
    "## Skill Frontmatter",
    "",
    "- `$ARGUMENTS` captures the command tail; `$0`, `$1`, ... map positional arguments.",
    "- `disable-model-invocation: true` marks manual-only skills or commands.",
    "- Honor `allowed-tools` as the maximum tool scope for a skill.",
  ];

  return `${lines.join("\n")}\n`;
}

function renderGeminiArtifacts(options = {}) {
  const bundle = getBundle(options);
  const settings = buildGeminiSettings(bundle, options);
  const policyToml = wrapGeneratedSection(
    buildGeminiPolicyToml(bundle, options),
    buildGeneratedMetadata(bundle, "policy", { artifact: "gemini-policy" }),
    "toml",
  );
  const instructionFile = wrapGeneratedSection(
    buildGeminiInstructions(bundle),
    buildGeneratedMetadata(bundle, "routing"),
    "markdown",
  );
  const settingsWithMeta = attachGeneratedMetadata(settings, buildGeneratedMetadata(bundle, "policy"));
  const capabilityGap = attachGeneratedMetadata(
    bundle.capability_gap_report,
    buildGeneratedMetadata(bundle, "policy", { artifact: "capability-gap" }),
  );
  const baseFiles = {
    "GEMINI.md": instructionFile,
    ".gemini/settings.json": `${JSON.stringify(settingsWithMeta, null, 2)}\n`,
    ".ai/workflow/capability-gap.json": `${JSON.stringify(capabilityGap, null, 2)}\n`,
    [GEMINI_POLICY_FILE]: `${policyToml}\n`,
  };
  const manifestFiles = [...Object.keys(baseFiles), ".ai/workflow/skills-manifest.json"];
  const skillsManifest = attachGeneratedMetadata(
    buildSkillsManifest({ bundle, generated_files: manifestFiles }),
    buildGeneratedMetadata(bundle, "skills", { artifact: "skills-manifest" }),
  );

  return {
    engine: "gemini_cli",
    bundle,
    settings: settingsWithMeta,
    files: {
      ...baseFiles,
      ".ai/workflow/skills-manifest.json": `${JSON.stringify(skillsManifest, null, 2)}\n`,
    },
  };
}

function writeGeminiArtifacts(options = {}) {
  const rendered = renderGeminiArtifacts(options);
  const outputRootDir = path.resolve(options.outputRootDir || resolveRootDir(options, rendered.bundle));
  const writtenFiles = [];

  for (const [relativePath, content] of Object.entries(rendered.files)) {
    const targetPath = path.join(outputRootDir, relativePath);
    ensureDir(path.dirname(targetPath));
    fs.writeFileSync(targetPath, content, "utf8");
    writtenFiles.push(normalizePath(path.relative(outputRootDir, targetPath)));
  }

  return {
    ...rendered,
    output_root: normalizePath(outputRootDir),
    written_files: writtenFiles,
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    outputRootDir: null,
    write: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--output":
        index += 1;
        parsed.outputRootDir = path.resolve(argv[index]);
        break;
      case "--write":
        parsed.write = true;
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
    if (parsed.write) {
      const result = writeGeminiArtifacts(parsed);
      console.log("GEMINI_ARTIFACTS_WRITTEN");
      for (const filePath of result.written_files) {
        console.log(filePath);
      }
      return;
    }

    const rendered = renderGeminiArtifacts(parsed);
    console.log(JSON.stringify({ engine: rendered.engine, files: Object.keys(rendered.files) }, null, 2));
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  buildGeminiInstructions,
  buildGeminiSettings,
  renderGeminiArtifacts,
  validateHierarchyContract,
  writeGeminiArtifacts,
};

if (require.main === module) {
  main();
}
