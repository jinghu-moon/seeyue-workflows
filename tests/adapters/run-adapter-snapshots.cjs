#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { dumpYamlFile, loadYamlFile } = require("../../scripts/runtime/yaml-loader.cjs");
const { writeClaudeCodeArtifacts } = require("../../scripts/adapters/claude-code.cjs");
const { writeCodexArtifacts } = require("../../scripts/adapters/codex.cjs");
const { renderGeminiArtifacts, writeGeminiArtifacts } = require("../../scripts/adapters/gemini-cli.cjs");
const { makeTempRoot } = require("../runtime/runtime-fixture-lib.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");
const workflowFiles = [
  "workflow/capabilities.yaml",
  "workflow/persona-bindings.yaml",
  "workflow/file-classes.yaml",
  "workflow/approval-matrix.yaml",
  "workflow/runtime.schema.yaml",
  "workflow/router.spec.yaml",
  "workflow/policy.spec.yaml",
];

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertHumanFacingRuntimeContracts(text, fileLabel) {
  assert(/approval requests MUST use runtime-approved zh-CN short actionable copy/i.test(text), `expected zh-CN approval contract in ${fileLabel}`);
  assert(/manual restore blockers MUST use runtime-approved zh-CN short actionable copy/i.test(text), `expected zh-CN restore blocker contract in ${fileLabel}`);
  assert(/`recommended_next` and `restore_reason` MUST come from runtime state/i.test(text), `expected runtime recovery authority contract in ${fileLabel}`);
  assert(/runtime approval request in zh-CN short actionable copy/i.test(text), `expected runtime approval request contract in ${fileLabel}`);
  assert(/runtime restore request in zh-CN short actionable copy/i.test(text), `expected runtime restore request contract in ${fileLabel}`);
}

function parseArgs(argv) {
  const parsed = {
    engine: null,
    caseName: null,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--engine":
        index += 1;
        parsed.engine = argv[index];
        break;
      case "--case":
        index += 1;
        parsed.caseName = argv[index];
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }

  if (!parsed.engine) {
    throw new Error("Missing required argument: --engine");
  }

  return parsed;
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function copyWorkflowFiles(rootDir) {
  for (const relativePath of workflowFiles) {
    const sourcePath = path.join(projectRoot, relativePath);
    const targetPath = path.join(rootDir, relativePath);
    dumpYamlFile(targetPath, loadYamlFile(sourcePath));
  }
}

function copyDirRecursive(sourceDir, targetDir) {
  if (!fs.existsSync(sourceDir)) {
    return;
  }
  ensureDir(targetDir);
  const entries = fs.readdirSync(sourceDir, { withFileTypes: true });
  for (const entry of entries) {
    const sourcePath = path.join(sourceDir, entry.name);
    const targetPath = path.join(targetDir, entry.name);
    if (entry.isDirectory()) {
      copyDirRecursive(sourcePath, targetPath);
      continue;
    }
    if (entry.isFile()) {
      ensureDir(path.dirname(targetPath));
      fs.copyFileSync(sourcePath, targetPath);
    }
  }
}

function copyHookFiles(rootDir, options = {}) {
  const omit = new Set(options.omit || []);
  const sourceDir = path.join(projectRoot, "scripts", "hooks");
  const targetDir = path.join(rootDir, "scripts", "hooks");
  ensureDir(targetDir);

  const entries = fs.readdirSync(sourceDir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isFile()) {
      continue;
    }
    if (omit.has(entry.name)) {
      continue;
    }
    fs.copyFileSync(path.join(sourceDir, entry.name), path.join(targetDir, entry.name));
  }
}

function copySkills(rootDir) {
  copyDirRecursive(path.join(projectRoot, ".agents", "skills"), path.join(rootDir, ".agents", "skills"));
}

function createAdapterFixtureRoot(prefix, options = {}) {
  const rootDir = makeTempRoot(prefix);
  copyWorkflowFiles(rootDir);
  if (options.includeHooks !== false) {
    copyHookFiles(rootDir, options);
  }
  if (options.includeSkills !== false) {
    copySkills(rootDir);
  }
  return rootDir;
}

function assertClaudeSettingsShape(settings) {
  assert(settings && typeof settings === "object", "expected settings object");
  assert(settings.hooks && typeof settings.hooks === "object", "expected settings.hooks");
  assert(Array.isArray(settings.hooks.SessionStart) && settings.hooks.SessionStart.length > 0, "expected SessionStart hook group");
  assert(Array.isArray(settings.hooks.UserPromptSubmit) && settings.hooks.UserPromptSubmit.length > 0, "expected UserPromptSubmit hook group");
  assert(Array.isArray(settings.hooks.PreToolUse) && settings.hooks.PreToolUse.some((entry) => entry.matcher === "Bash"), "expected PreToolUse Bash matcher");
  assert(Array.isArray(settings.hooks.PreToolUse) && settings.hooks.PreToolUse.some((entry) => entry.matcher === "Write|Edit"), "expected PreToolUse Write|Edit matcher");
  assert(Array.isArray(settings.hooks.PostToolUse) && settings.hooks.PostToolUse.some((entry) => entry.matcher === "Write|Edit"), "expected PostToolUse Write|Edit matcher");
  assert(Array.isArray(settings.hooks.PostToolUse) && settings.hooks.PostToolUse.some((entry) => entry.matcher === "Bash"), "expected PostToolUse Bash matcher");
  assert(Array.isArray(settings.hooks.Stop) && settings.hooks.Stop.length > 0, "expected Stop hook group");
}

function assertGeminiSettingsShape(settings) {
  assert(settings && typeof settings === "object", "expected settings object");
  assert(settings.context && Array.isArray(settings.context.fileName), "expected settings.context.fileName array");
  assert(settings.general && settings.general.defaultApprovalMode === "default", "expected safe default approval mode");
  assert(settings.general && settings.general.checkpointing && settings.general.checkpointing.enabled === true, "expected checkpointing enabled");
  assert(settings.skills && settings.skills.enabled === true, "expected skills enabled");
  assert(settings.hooks && typeof settings.hooks === "object", "expected hooks object");
  assert(Array.isArray(settings.hooks.SessionStart) && settings.hooks.SessionStart.length > 0, "expected SessionStart hook group");
  assert(Array.isArray(settings.hooks.BeforeAgent) && settings.hooks.BeforeAgent.length > 0, "expected BeforeAgent hook group");
  assert(Array.isArray(settings.hooks.BeforeToolSelection) && settings.hooks.BeforeToolSelection.length > 0, "expected BeforeToolSelection hook group");
  assert(Array.isArray(settings.hooks.BeforeTool) && settings.hooks.BeforeTool.some((entry) => entry.matcher === "run_shell_command"), "expected BeforeTool shell matcher");
  assert(Array.isArray(settings.hooks.BeforeTool) && settings.hooks.BeforeTool.some((entry) => entry.matcher === "write_file|replace"), "expected BeforeTool write matcher");
  assert(Array.isArray(settings.hooks.AfterTool) && settings.hooks.AfterTool.some((entry) => entry.matcher === "run_shell_command"), "expected AfterTool shell matcher");
  assert(Array.isArray(settings.hooks.AfterTool) && settings.hooks.AfterTool.some((entry) => entry.matcher === "write_file|replace"), "expected AfterTool write matcher");
  assert(Array.isArray(settings.hooks.AfterModel) && settings.hooks.AfterModel.length > 0, "expected AfterModel hook group");
  assert(Array.isArray(settings.hooks.AfterAgent) && settings.hooks.AfterAgent.length > 0, "expected AfterAgent hook group");
}

function runClaudeCodeMissingHookBinding() {
  const rootDir = createAdapterFixtureRoot("adapter-claude-missing-hook-", {
    omit: ["sy-stop.cjs"],
  });

  let failed = false;
  try {
    writeClaudeCodeArtifacts({ rootDir, outputRootDir: rootDir });
  } catch (error) {
    failed = true;
    assert(
      /ADAPTER_MISSING_REQUIRED_HOOK|sy-stop\.cjs|event=Stop/i.test(String(error.message || "")),
      `expected missing hook failure but got ${JSON.stringify(error.message)}`,
    );
  }

  assert(failed === true, "expected adapter to fail when a required hook file is missing");
}

function runClaudeCodeSnapshot() {
  const outputRootDir = makeTempRoot("adapter-claude-output-");
  const result = writeClaudeCodeArtifacts({
    rootDir: projectRoot,
    outputRootDir,
  });

  const claudePath = path.join(outputRootDir, "CLAUDE.md");
  const settingsPath = path.join(outputRootDir, ".claude", "settings.json");

  assert(fs.existsSync(claudePath), `expected artifact ${claudePath}`);
  assert(fs.existsSync(settingsPath), `expected artifact ${settingsPath}`);
  assert(Array.isArray(result.written_files) && result.written_files.includes("CLAUDE.md"), `expected written CLAUDE.md but got ${JSON.stringify(result.written_files)}`);

  const claudeText = fs.readFileSync(claudePath, "utf8");
  const settings = JSON.parse(fs.readFileSync(settingsPath, "utf8"));

  assert(/Generated artifact/i.test(claudeText), "expected generated artifact notice in CLAUDE.md");
  assert(/source of truth/i.test(claudeText), "expected source of truth notice in CLAUDE.md");
  assert(/English/i.test(claudeText) && /zh-CN/i.test(claudeText), "expected English and zh-CN language policy in CLAUDE.md");
  assert(/Hook Summary/i.test(claudeText), "expected hook summary in CLAUDE.md");
  assert(/Router Summary/i.test(claudeText), "expected router summary in CLAUDE.md");
  assert(/Approval Summary/i.test(claudeText), "expected approval summary in CLAUDE.md");
  assertHumanFacingRuntimeContracts(claudeText, "CLAUDE.md");

  assertClaudeSettingsShape(settings);
}

function runCodexMissingSkillMetadata() {
  const rootDir = createAdapterFixtureRoot("adapter-codex-missing-skill-", {
    includeSkills: false,
    includeHooks: false,
  });

  let failed = false;
  try {
    writeCodexArtifacts({ rootDir, outputRootDir: rootDir });
  } catch (error) {
    failed = true;
    assert(
      /CODEX_ADAPTER_MISSING_SKILL_METADATA|skill_root|skill_file/i.test(String(error.message || "")),
      `expected missing skill metadata failure but got ${JSON.stringify(error.message)}`,
    );
  }

  assert(failed === true, "expected codex adapter to fail when skill metadata cannot be compiled");
}

function runCodexSnapshot() {
  const outputRootDir = makeTempRoot("adapter-codex-output-");
  const result = writeCodexArtifacts({
    rootDir: projectRoot,
    outputRootDir,
  });

  const agentsPath = path.join(outputRootDir, "AGENTS.md");
  const configPath = path.join(outputRootDir, ".codex", "config.toml");
  const skillMetadataPath = path.join(outputRootDir, ".codex", "skill-metadata.json");

  assert(fs.existsSync(agentsPath), `expected artifact ${agentsPath}`);
  assert(fs.existsSync(configPath), `expected artifact ${configPath}`);
  assert(fs.existsSync(skillMetadataPath), `expected artifact ${skillMetadataPath}`);
  assert(Array.isArray(result.written_files) && result.written_files.includes("AGENTS.md"), `expected written AGENTS.md but got ${JSON.stringify(result.written_files)}`);

  const agentsText = fs.readFileSync(agentsPath, "utf8");
  const configText = fs.readFileSync(configPath, "utf8");
  const skillMetadata = JSON.parse(fs.readFileSync(skillMetadataPath, "utf8"));

  assert(/Generated artifact/i.test(agentsText), "expected generated artifact notice in AGENTS.md");
  assert(/source of truth/i.test(agentsText), "expected source of truth notice in AGENTS.md");
  assert(/English/i.test(agentsText) && /zh-CN/i.test(agentsText), "expected English and zh-CN language policy in AGENTS.md");
  assert(/child_agents_md/i.test(agentsText), "expected child_agents_md layering note in AGENTS.md");
  assert(/Skills/i.test(agentsText), "expected skills section in AGENTS.md");
  assert(/Approval And Sandbox/i.test(agentsText), "expected approval and sandbox section in AGENTS.md");
  assertHumanFacingRuntimeContracts(agentsText, "AGENTS.md");

  assert(/approval_policy = "on-request"/.test(configText), "expected on-request approval policy in config.toml");
  assert(/sandbox_mode = "workspace-write"/.test(configText), "expected workspace-write sandbox in config.toml");
  assert(/\[features\]/.test(configText), "expected features table in config.toml");
  assert(/child_agents_md = true/.test(configText), "expected child_agents_md flag in config.toml");
  assert(/web_search_request = true/.test(configText), "expected web_search_request flag in config.toml");
  assert(/\[history\]/.test(configText) && /persistence = "save-all"/.test(configText), "expected history persistence in config.toml");

  assert(Array.isArray(skillMetadata.skills) && skillMetadata.skills.length > 0, "expected non-empty skill metadata list");
  assert(skillMetadata.progressive_disclosure === true, "expected progressive disclosure skill metadata");
  assert(skillMetadata.skills.some((entry) => entry.name === "sy-workflow"), "expected sy-workflow in skill metadata");
  assert(skillMetadata.skills.some((entry) => entry.name === "sy-constraints"), "expected sy-constraints in skill metadata");

  const workflowSkill = skillMetadata.skills.find((entry) => entry.name === "sy-workflow");
  assert(workflowSkill && Array.isArray(workflowSkill.allowed_tools), "expected allowed_tools on sy-workflow metadata");
  assert(workflowSkill.allowed_tools.includes("Read"), `expected Read in sy-workflow allowed_tools but got ${JSON.stringify(workflowSkill && workflowSkill.allowed_tools)}`);
}

function runGeminiHierarchyLoss() {
  const { compileAdapterBundle } = require("../../scripts/adapters/compile-adapter.cjs");
  const broken = compileAdapterBundle({ rootDir: projectRoot, engine: "gemini_cli" });
  broken.engine_contract.hierarchy_mode = "workspace_only";

  let failed = false;
  try {
    renderGeminiArtifacts({ bundle: broken });
  } catch (error) {
    failed = true;
    assert(
      /GEMINI_ADAPTER_HIERARCHY_LOSS|hierarchy_mode|workspace_only/i.test(String(error.message || "")),
      `expected hierarchy loss failure but got ${JSON.stringify(error.message)}`,
    );
  }

  assert(failed === true, "expected gemini adapter to fail when hierarchy contract is lost");
}

function runGeminiSnapshot() {
  const outputRootDir = makeTempRoot("adapter-gemini-output-");
  const result = writeGeminiArtifacts({
    rootDir: projectRoot,
    outputRootDir,
  });

  const memoryPath = path.join(outputRootDir, "GEMINI.md");
  const settingsPath = path.join(outputRootDir, ".gemini", "settings.json");
  const policyPath = path.join(outputRootDir, ".gemini", "policies", "seeyue-workflows.toml");

  assert(fs.existsSync(memoryPath), `expected artifact ${memoryPath}`);
  assert(fs.existsSync(settingsPath), `expected artifact ${settingsPath}`);
  assert(fs.existsSync(policyPath), `expected artifact ${policyPath}`);
  assert(Array.isArray(result.written_files) && result.written_files.includes("GEMINI.md"), `expected written GEMINI.md but got ${JSON.stringify(result.written_files)}`);

  const memoryText = fs.readFileSync(memoryPath, "utf8");
  const settings = JSON.parse(fs.readFileSync(settingsPath, "utf8"));
  const policyText = fs.readFileSync(policyPath, "utf8");

  assert(/Generated artifact/i.test(memoryText), "expected generated artifact notice in GEMINI.md");
  assert(/source of truth/i.test(memoryText), "expected source of truth notice in GEMINI.md");
  assert(/global `~\/\.gemini\/GEMINI\.md`/i.test(memoryText), "expected hierarchy note in GEMINI.md");
  assert(/Project settings in `\.gemini\/settings\.json` override user and system settings/i.test(memoryText), "expected settings precedence note in GEMINI.md");
  assert(/Hooks And Approval/i.test(memoryText), "expected hooks and approval section in GEMINI.md");
  assert(/read-only/i.test(memoryText), "expected planning mode read-only note in GEMINI.md");
  assert(/restore/i.test(memoryText), "expected checkpoint restore note in GEMINI.md");
  assert(/English/i.test(memoryText) && /zh-CN/i.test(memoryText), "expected English and zh-CN language policy in GEMINI.md");
  assertHumanFacingRuntimeContracts(memoryText, "GEMINI.md");
  assert(/SY:GENERATED:BEGIN/.test(policyText), "expected generated marker in gemini policy");
  assert(/\[\[rule\]\]/.test(policyText), "expected policy rules in gemini policy");
  assert(/decision\s*=\s*\"allow\"/.test(policyText), "expected allow decision in gemini policy");
  assert(/decision\s*=\s*\"ask_user\"/.test(policyText), "expected ask_user decision in gemini policy");
  assert(/decision\s*=\s*\"deny\"/.test(policyText), "expected deny decision in gemini policy");

  assertGeminiSettingsShape(settings);
  assert(settings.context.fileName.includes("GEMINI.md"), "expected GEMINI.md in settings.context.fileName");
  assert(settings.hooks.BeforeTool.every((entry) => Array.isArray(entry.hooks) && entry.hooks.every((hook) => /gemini-hook-bridge\.cjs/.test(String(hook.command || "")))), "expected bridge-based BeforeTool hook commands");
  assert(settings.hooks.AfterAgent.every((entry) => Array.isArray(entry.hooks) && entry.hooks.some((hook) => /sy-stop\.cjs/.test(String(hook.command || "")))), "expected sy-stop bridge in AfterAgent hooks");
}

function writeArtifactsByEngine(engine, outputRootDir) {
  if (engine === "claude_code") {
    return writeClaudeCodeArtifacts({ rootDir: projectRoot, outputRootDir });
  }
  if (engine === "codex") {
    return writeCodexArtifacts({ rootDir: projectRoot, outputRootDir });
  }
  if (engine === "gemini_cli") {
    return writeGeminiArtifacts({ rootDir: projectRoot, outputRootDir });
  }
  throw new Error(`UNSUPPORTED_ENGINE ${engine}`);
}

function runSkillsManifestSnapshot(engine) {
  const outputRootDir = makeTempRoot(`adapter-skills-manifest-${engine}-`);
  const result = writeArtifactsByEngine(engine, outputRootDir);
  const manifestPath = path.join(outputRootDir, ".ai", "workflow", "skills-manifest.json");
  assert(fs.existsSync(manifestPath), `expected skills manifest at ${manifestPath}`);

  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  assert(manifest && typeof manifest === "object", "expected manifest object");
  assert(manifest.schema_kind === "skills_manifest", `expected schema_kind=skills_manifest but got ${manifest.schema_kind}`);
  assert(manifest._sy_generated && typeof manifest._sy_generated === "object", "expected _sy_generated metadata on skills manifest");
  assert(typeof manifest.spec_hash === "string" && manifest.spec_hash.length > 0, "expected spec_hash in skills manifest");
  assert(Array.isArray(manifest.generated_files) && manifest.generated_files.length > 0, "expected generated_files list");
  assert(
    manifest.generated_files.some((file) => /skills-manifest\.json$/.test(file)),
    "expected skills-manifest.json listed in generated_files",
  );
  assert(
    manifest.generated_files.some((file) => /capability-gap\.json$/.test(file)),
    "expected capability-gap.json listed in generated_files",
  );

  if (engine === "claude_code") {
    assert(manifest.generated_files.some((file) => /CLAUDE\.md$/.test(file)), "expected CLAUDE.md listed in generated_files");
  }
  if (engine === "codex") {
    assert(manifest.generated_files.some((file) => /AGENTS\.md$/.test(file)), "expected AGENTS.md listed in generated_files");
    assert(manifest.generated_files.some((file) => /skill-metadata\.json$/.test(file)), "expected skill-metadata.json listed in generated_files");
  }
  if (engine === "gemini_cli") {
    assert(manifest.generated_files.some((file) => /GEMINI\.md$/.test(file)), "expected GEMINI.md listed in generated_files");
  }

  assert(Array.isArray(result.written_files), "expected written_files list from adapter");
  assert(result.written_files.includes(".ai/workflow/skills-manifest.json"), "expected skills-manifest.json in written_files");
}

const ENGINE_CASES = {
  claude_code: {
    passSignal: "CLAUDE_CODE_ADAPTER_PASS",
    defaultRunner: runClaudeCodeSnapshot,
    cases: {
      "missing-hook-binding": runClaudeCodeMissingHookBinding,
      "skills-manifest": () => runSkillsManifestSnapshot("claude_code"),
    },
  },
  codex: {
    passSignal: "CODEX_ADAPTER_PASS",
    defaultRunner: runCodexSnapshot,
    cases: {
      "missing-skill-metadata": runCodexMissingSkillMetadata,
      "skills-manifest": () => runSkillsManifestSnapshot("codex"),
    },
  },
  gemini_cli: {
    passSignal: "GEMINI_ADAPTER_PASS",
    defaultRunner: runGeminiSnapshot,
    cases: {
      "hierarchy-loss": runGeminiHierarchyLoss,
      "skills-manifest": () => runSkillsManifestSnapshot("gemini_cli"),
    },
  },
};

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  const engineCases = ENGINE_CASES[parsed.engine];
  if (!engineCases) {
    console.error(`UNSUPPORTED_ENGINE ${parsed.engine}`);
    process.exit(1);
  }

  const runner = parsed.caseName ? engineCases.cases[parsed.caseName] : engineCases.defaultRunner;
  if (typeof runner !== "function") {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }

  try {
    runner();
    console.log(`CASE_PASS ${parsed.caseName || parsed.engine}`);
    console.log(engineCases.passSignal);
  } catch (error) {
    console.error(`CASE_FAIL ${parsed.caseName || parsed.engine}`);
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

main();
