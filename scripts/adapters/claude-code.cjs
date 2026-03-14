#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { compileAdapterBundle } = require("./compile-adapter.cjs");
const {
  attachGeneratedMetadata,
  buildGeneratedMetadata,
  detectSeededFormat,
  mergeSeededSections,
  wrapGeneratedSection,
} = require("./adapter-utils.cjs");
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

const GROUP_METADATA = {
  "SessionStart:*": {
    description: "SY bootstrap: inject first-turn workflow + constraint routing context",
    summary: "`SessionStart`: bootstrap workflow + constraint routing.",
  },
  "UserPromptSubmit:*": {
    description: "SY refresh: lightweight constraint anchor for long-session drift",
    summary: "`UserPromptSubmit`: refresh the long-session anchor.",
  },
  "PreToolUse:Bash": {
    description: "SY guard: command class approval + destructive guard + loop budgets",
    summary: "`PreToolUse(Bash)`: command class approval and loop-budget guard.",
  },
  "PreToolUse:Write|Edit": {
    description: "SY guard: pre-write policy gates + session state integrity",
    summary: "`PreToolUse(Write|Edit)`: TDD, secret, protected-file, and session-integrity guard.",
  },
  "PostToolUse:Write|Edit": {
    description: "SY post-write: audit log, index invalidation, and scope drift warning",
    summary: "`PostToolUse(Write|Edit)`: capture write evidence and scope drift.",
  },
  "PostToolUse:Bash": {
    description: "SY post-bash: capture verification + TDD red/green evidence",
    summary: "`PostToolUse(Bash)`: capture verification and TDD evidence.",
  },
  "Stop:*": {
    description: "SY gate: block incomplete checkpoints before response stop",
    summary: "`Stop`: checkpoint and resume-frontier gate.",
  },
};

const HOOK_TIMEOUTS = {
  "scripts/hooks/sy-session-start.cjs": 8,
  "scripts/hooks/sy-prompt-refresh.cjs": 6,
  "scripts/hooks/sy-pretool-bash.cjs": 10,
  "scripts/hooks/sy-pretool-bash-budget.cjs": 8,
  "scripts/hooks/sy-pretool-write.cjs": 10,
  "scripts/hooks/sy-pretool-write-session.cjs": 8,
  "scripts/hooks/sy-posttool-write.cjs": 8,
  "scripts/hooks/sy-posttool-bash-verify.cjs": 8,
  "scripts/hooks/sy-stop.cjs": 10,
};

const EVENT_ORDER = ["SessionStart", "UserPromptSubmit", "PreToolUse", "PostToolUse", "Stop"];

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function parseHookEvent(eventName) {
  const [event, matcher] = String(eventName || "").split(":");
  return {
    event,
    matcher: matcher || "*",
    groupKey: `${event}:${matcher || "*"}`,
  };
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
  return compileAdapterBundle({ rootDir: options.rootDir, engine: "claude_code" });
}

function assertClaudeBundle(bundle) {
  if (!bundle || bundle.engine !== "claude_code") {
    throw new Error(`CLAUDE_ADAPTER_ENGINE_MISMATCH expected=claude_code actual=${JSON.stringify(bundle && bundle.engine)}`);
  }
}

function ensureRequiredHooksPresent(bundle, rootDir) {
  assertClaudeBundle(bundle);
  for (const hook of bundle.hook_contract.hooks) {
    if (hook.required !== true) {
      continue;
    }
    const absolutePath = path.join(rootDir, hook.script);
    if (!fs.existsSync(absolutePath)) {
      throw new Error(
        `ADAPTER_MISSING_REQUIRED_HOOK event=${hook.event} script=${normalizePath(hook.script)}`,
      );
    }
  }
}

function renderHookCommand(scriptPath) {
  return `node ${normalizePath(scriptPath)}`;
}

function buildClaudeSettings(bundle, options = {}) {
  const rootDir = resolveRootDir(options, bundle);
  ensureRequiredHooksPresent(bundle, rootDir);

  const grouped = new Map();
  for (const hook of bundle.hook_contract.hooks) {
    const parsed = parseHookEvent(hook.event);
    const mapKey = parsed.groupKey;
    if (!grouped.has(mapKey)) {
      const metadata = GROUP_METADATA[mapKey] || {
        description: `SY generated hook group for ${mapKey}`,
      };
      grouped.set(mapKey, {
        event: parsed.event,
        matcher: parsed.matcher,
        description: metadata.description,
        hooks: [],
      });
    }
    grouped.get(mapKey).hooks.push({
      type: "command",
      command: renderHookCommand(hook.script),
      timeout: HOOK_TIMEOUTS[hook.script] || 8,
    });
  }

  const settings = { hooks: {} };
  for (const eventName of EVENT_ORDER) {
    settings.hooks[eventName] = [];
  }

  for (const group of grouped.values()) {
    if (!settings.hooks[group.event]) {
      continue;
    }
    settings.hooks[group.event].push({
      matcher: group.matcher,
      hooks: group.hooks,
      description: group.description,
    });
  }

  return settings;
}

function renderRuleLanguageSummary(bundle) {
  const agentLanguage = bundle.language_policy.agent_rule_language === "en"
    ? "English"
    : bundle.language_policy.agent_rule_language;
  return [
    `- Write machine-facing rules, contracts, plans, and skill logic in ${agentLanguage}.`,
    `- Write human-facing approvals, blockers, and status updates in concise ${bundle.language_policy.human_output_language}.`,
    "- Keep approval text short, explicit, and action-oriented.",
    "- Human-facing approval requests MUST use runtime-approved zh-CN short actionable copy.",
    "- Human-facing manual restore blockers MUST use runtime-approved zh-CN short actionable copy.",
  ];
}

function renderApprovalSummary(bundle) {
  const commandClasses = bundle.approval_contract.approval_required_for.command_classes.map((entry) => `\`${entry}\``).join(", ");
  const fileClasses = bundle.approval_contract.approval_required_for.file_classes.map((entry) => `\`${entry}\``).join(", ");
  const notifyOnly = bundle.approval_contract.notify_only.allowed_change_classes.map((entry) => `\`${entry}\``).join(", ");
  return [
    `- Human approval is required for command classes: ${commandClasses}.`,
    `- Human approval is required for file classes: ${fileClasses}.`,
    `- Notify-only relief is limited to low-risk change classes: ${notifyOnly}.`,
  ];
}

function renderHookSummary() {
  return Object.values(GROUP_METADATA).map((entry) => `- ${entry.summary}`);
}

function buildClaudeInstructions(bundle) {
  assertClaudeBundle(bundle);
  const reviewChain = bundle.review_chain.map((entry) => `\`${entry}\``).join(" -> ");
  const sourceOfTruth = SOURCE_OF_TRUTH_FILES.map((entry) => `- \`${entry}\``);
  const hookSummary = renderHookSummary();

  const lines = [
    "# CLAUDE.md",
    "",
    "> Generated artifact for `claude_code`. Do not edit manually.",
    "> Vendor files are deployment artifacts. `workflow/*.yaml` remains the machine source of truth.",
    "",
    "## Mission",
    "",
    "- Use this repository as the runtime and adapter source for `seeyue-workflows`.",
    "- Prefer durable workflow state under `.ai/workflow/` over chat memory or free-form recap.",
    "- Keep context narrow and load only the minimum skill or document scope needed for the active node.",
    "",
    "## Language Policy",
    "",
    ...renderRuleLanguageSummary(bundle),
    "",
    "## Source Of Truth",
    "",
    ...sourceOfTruth,
    "",
    "## Router Summary",
    "",
    "- Execution is state-first and blocker-first.",
    "- V4 Phase 1 uses a single active phase and a single active node.",
    `- Default review chain: ${reviewChain}.`,
    "- `recommended_next` and `restore_reason` MUST come from runtime state, not free-form chat reasoning.",
    "",
    "## Approval Summary",
    "",
    ...renderApprovalSummary(bundle),
    "- If runtime enters `approval_pending`, surface the runtime approval request in zh-CN short actionable copy and wait.",
    "",
    "## Hook Summary",
    "",
    ...hookSummary,
    "",
    "## Recovery Summary",
    "",
    "- If runtime enters `restore_pending`, resolve recovery before any new write or command.",
    "- If manual intervention is required, surface the runtime restore request in zh-CN short actionable copy and stop.",
    "- Treat runtime recovery state as authoritative over chat recap when the two diverge.",
    "",
    "## Skills And Personas",
    "",
    "- Load skills from `.agents/skills` with progressive disclosure only.",
    "- Keep reviewer personas isolated from the author context.",
    "- Do not bypass router, policy, or hook boundaries through ad-hoc role switching.",
    "",
    "## Skill Frontmatter",
    "",
    "- `$ARGUMENTS` captures the command tail; `$0`, `$1`, ... map positional arguments.",
    "- `disable-model-invocation: true` marks manual-only skills or commands.",
    "- Honor `allowed-tools` as the maximum tool scope for a skill.",
  ];

  return `${lines.join("\n")}\n`;
}

function renderClaudeCodeArtifacts(options = {}) {
  const bundle = getBundle(options);
  const settings = buildClaudeSettings(bundle, options);
  const instructionFile = wrapGeneratedSection(
    buildClaudeInstructions(bundle),
    buildGeneratedMetadata(bundle, "routing"),
    "markdown",
  );
  const settingsWithMeta = attachGeneratedMetadata(settings, buildGeneratedMetadata(bundle, "policy"));
  const capabilityGap = attachGeneratedMetadata(
    bundle.capability_gap_report,
    buildGeneratedMetadata(bundle, "policy", { artifact: "capability-gap" }),
  );
  const baseFiles = {
    "CLAUDE.md": instructionFile,
    ".claude/settings.json": `${JSON.stringify(settingsWithMeta, null, 2)}\n`,
    ".ai/workflow/capability-gap.json": `${JSON.stringify(capabilityGap, null, 2)}\n`,
  };
  const manifestFiles = [...Object.keys(baseFiles), ".ai/workflow/skills-manifest.json"];
  const skillsManifest = attachGeneratedMetadata(
    buildSkillsManifest({ bundle, generated_files: manifestFiles }),
    buildGeneratedMetadata(bundle, "skills", { artifact: "skills-manifest" }),
  );

  return {
    engine: "claude_code",
    bundle,
    files: {
      ...baseFiles,
      ".ai/workflow/skills-manifest.json": `${JSON.stringify(skillsManifest, null, 2)}\n`,
    },
    settings: settingsWithMeta,
  };
}

function writeClaudeCodeArtifacts(options = {}) {
  const rendered = renderClaudeCodeArtifacts(options);
  const outputRootDir = path.resolve(options.outputRootDir || resolveRootDir(options, rendered.bundle));
  const writtenFiles = [];

  for (const [relativePath, content] of Object.entries(rendered.files)) {
    const targetPath = path.join(outputRootDir, relativePath);
    ensureDir(path.dirname(targetPath));
    let nextContent = content;
    const seededFormat = detectSeededFormat(relativePath);
    if (seededFormat) {
      const existing = fs.existsSync(targetPath) ? fs.readFileSync(targetPath, "utf8") : null;
      nextContent = mergeSeededSections(existing, content, seededFormat);
    }
    fs.writeFileSync(targetPath, nextContent, "utf8");
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
      const result = writeClaudeCodeArtifacts(parsed);
      console.log("CLAUDE_CODE_ARTIFACTS_WRITTEN");
      for (const filePath of result.written_files) {
        console.log(filePath);
      }
      return;
    }

    const rendered = renderClaudeCodeArtifacts(parsed);
    console.log(
      JSON.stringify(
        {
          engine: rendered.engine,
          files: Object.keys(rendered.files),
        },
        null,
        2,
      ),
    );
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  buildClaudeCodeSettings: buildClaudeSettings,
  buildClaudeInstructions,
  renderClaudeCodeArtifacts,
  writeClaudeCodeArtifacts,
};

if (require.main === module) {
  main();
}
