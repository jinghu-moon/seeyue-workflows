#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { compileAdapterBundle, compileAllAdapters } = require("../../scripts/adapters/compile-adapter.cjs");
const { dumpYamlFile, loadYamlFile } = require("../../scripts/runtime/yaml-loader.cjs");
const { makeTempRoot } = require("../runtime/runtime-fixture-lib.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");
const workflowFiles = [
  "workflow/capabilities.yaml",
  "workflow/persona-bindings.yaml",
  "workflow/file-classes.yaml",
  "workflow/approval-matrix.yaml",
  "workflow/hooks.spec.yaml",
  "workflow/runtime.schema.yaml",
  "workflow/router.spec.yaml",
  "workflow/policy.spec.yaml",
];

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parseArgs(argv) {
  const parsed = { caseName: null };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--case") {
      index += 1;
      parsed.caseName = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${argv[index]}`);
  }
  return parsed;
}

function createCompilerFixtureRoot(prefix) {
  const rootDir = makeTempRoot(prefix);
  for (const relativePath of workflowFiles) {
    const sourcePath = path.join(projectRoot, relativePath);
    const targetPath = path.join(rootDir, relativePath);
    dumpYamlFile(targetPath, loadYamlFile(sourcePath));
  }
  return rootDir;
}

function runMissingCapabilityBinding() {
  const rootDir = createCompilerFixtureRoot("adapter-compiler-missing-cap-");
  const personaBindingsPath = path.join(rootDir, "workflow", "persona-bindings.yaml");
  const personaBindings = loadYamlFile(personaBindingsPath);
  personaBindings.personas.author.allowed_capabilities = [];
  dumpYamlFile(personaBindingsPath, personaBindings);

  let failed = false;
  try {
    compileAdapterBundle({ rootDir, engine: "claude_code" });
  } catch (error) {
    failed = true;
    assert(/CAPABILITY|code_edit|author/i.test(String(error.message || "")), `expected capability binding failure but got ${JSON.stringify(error.message)}`);
  }
  assert(failed === true, "expected compiler to fail when default capability binding is missing");
}

function runCompileClaudeCode() {
  const bundle = compileAdapterBundle({ rootDir: projectRoot, engine: "claude_code" });
  assert(bundle.engine === "claude_code", `expected engine=claude_code but got ${JSON.stringify(bundle.engine)}`);
  assert(bundle.render_targets.instruction_file === "CLAUDE.md", `expected CLAUDE.md but got ${JSON.stringify(bundle.render_targets)}`);
  assert(bundle.engine_contract.native_hook_support === true, "expected native_hook_support=true");
  assert(bundle.render_targets.config_files.includes(".claude/settings.json"), `expected .claude/settings.json but got ${JSON.stringify(bundle.render_targets.config_files)}`);
  assert(bundle.hook_contract.hooks.length >= 5, `expected hook contracts but got ${JSON.stringify(bundle.hook_contract)}`);
}

function runCompileCodex() {
  const bundle = compileAdapterBundle({ rootDir: projectRoot, engine: "codex" });
  assert(bundle.engine === "codex", `expected engine=codex but got ${JSON.stringify(bundle.engine)}`);
  assert(bundle.render_targets.instruction_file === "AGENTS.md", `expected AGENTS.md but got ${JSON.stringify(bundle.render_targets)}`);
  assert(bundle.engine_contract.native_hook_support === false, "expected native_hook_support=false for codex");
  assert(bundle.render_targets.config_files.includes(".codex/config.toml"), `expected .codex/config.toml but got ${JSON.stringify(bundle.render_targets.config_files)}`);
  assert(bundle.engine_contract.context_strategy === "hierarchical_agents_md", `expected codex context strategy but got ${JSON.stringify(bundle.engine_contract)}`);
}

function runCompileGemini() {
  const bundle = compileAdapterBundle({ rootDir: projectRoot, engine: "gemini_cli" });
  assert(bundle.engine === "gemini_cli", `expected engine=gemini_cli but got ${JSON.stringify(bundle.engine)}`);
  assert(bundle.render_targets.instruction_file === "GEMINI.md", `expected GEMINI.md but got ${JSON.stringify(bundle.render_targets)}`);
  assert(bundle.engine_contract.native_hook_support === true, "expected native_hook_support=true for gemini_cli");
  assert(bundle.render_targets.config_files.includes(".gemini/settings.json"), `expected .gemini/settings.json but got ${JSON.stringify(bundle.render_targets.config_files)}`);
  assert(bundle.engine_contract.hook_strategy === "native_settings_hooks", `expected native_settings_hooks but got ${JSON.stringify(bundle.engine_contract)}`);
  assert(bundle.engine_contract.approval_surface === "settings_hooks_and_approval_mode", `expected settings_hooks_and_approval_mode but got ${JSON.stringify(bundle.engine_contract)}`);
}

function runCompileAll() {
  const bundles = compileAllAdapters({ rootDir: projectRoot });
  assert(Array.isArray(bundles) && bundles.length === 3, `expected 3 bundles but got ${JSON.stringify(bundles)}`);
  const engines = bundles.map((bundle) => bundle.engine).sort();
  assert(JSON.stringify(engines) === JSON.stringify(["claude_code", "codex", "gemini_cli"]), `unexpected engines ${JSON.stringify(engines)}`);
}

function runCompilePasses() {
  const bundle = compileAdapterBundle({ rootDir: projectRoot, engine: "claude_code" });
  assert(bundle.passes, "expected passes on adapter bundle");
  assert(bundle.passes.routing.schema_kind === "adapter_routing_pass", "expected routing pass schema");
  assert(bundle.passes.skills.schema_kind === "adapter_skill_pass", "expected skill pass schema");
  assert(bundle.passes.policy.schema_kind === "adapter_policy_pass", "expected policy pass schema");
  const skillIds = (bundle.passes.skills.skill_registry.skills || []).map((skill) => skill.skill_id);
  assert(skillIds.includes("sy-workflow"), "expected sy-workflow in skill registry");
}

const CASES = {
  "missing-capability-binding": runMissingCapabilityBinding,
  "compile-claude-code": runCompileClaudeCode,
  "compile-codex": runCompileCodex,
  "compile-gemini": runCompileGemini,
  "compile-all": runCompileAll,
  "compile-passes": runCompilePasses,
};

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  const selected = parsed.caseName ? [[parsed.caseName, CASES[parsed.caseName]]] : Object.entries(CASES);
  if (selected.some(([, runner]) => typeof runner !== "function")) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }

  let failed = false;
  for (const [caseName, runner] of selected) {
    try {
      runner();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
      break;
    }
  }

  if (failed) {
    process.exit(1);
  }

  console.log("ADAPTER_COMPILER_PASS");
}

main();
