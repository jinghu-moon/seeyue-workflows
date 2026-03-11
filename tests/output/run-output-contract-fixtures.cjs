#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { loadYamlFile } = require("../../scripts/runtime/yaml-loader.cjs");
const { compileAdapterBundle } = require("../../scripts/adapters/compile-adapter.cjs");
const { validateOutputEntries } = require("../../scripts/runtime/validate-output.cjs");
const { appendOutputLog, readOutputLog } = require("../../scripts/runtime/output-log.cjs");
const { persistOutputTemplatesForTest } = require("../../scripts/runtime/hook-client.cjs");

const { validateWorkflowSpecs } = require("../../scripts/runtime/spec-validator.cjs");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-output-contract-"));
}

function copyFileSync(sourcePath, targetPath) {
  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.copyFileSync(sourcePath, targetPath);
}

function copyDirRecursive(sourceDir, targetDir) {
  if (!fs.existsSync(sourceDir)) {
    return;
  }
  fs.mkdirSync(targetDir, { recursive: true });
  const entries = fs.readdirSync(sourceDir, { withFileTypes: true });
  for (const entry of entries) {
    const sourcePath = path.join(sourceDir, entry.name);
    const targetPath = path.join(targetDir, entry.name);
    if (entry.isDirectory()) {
      copyDirRecursive(sourcePath, targetPath);
      continue;
    }
    if (entry.isFile()) {
      copyFileSync(sourcePath, targetPath);
    }
  }
}

function createFixtureRoot() {
  const rootDir = makeTempRoot();
  const workflowFiles = [
    "workflow/skills.spec.yaml",
    "workflow/output-templates.spec.yaml",
    "workflow/capabilities.yaml",
    "workflow/persona-bindings.yaml",
    "workflow/file-classes.yaml",
    "workflow/approval-matrix.yaml",
    "workflow/runtime.schema.yaml",
    "workflow/router.spec.yaml",
    "workflow/policy.spec.yaml",
    "workflow/hooks.spec.yaml",
    "workflow/hook-contract.schema.yaml",
    "workflow/validate-manifest.yaml",
  ];

  for (const rel of workflowFiles) {
    const source = path.join(process.cwd(), rel);
    const target = path.join(rootDir, rel);
    copyFileSync(source, target);
  }

  copyDirRecursive(path.join(process.cwd(), "scripts", "runtime"), path.join(rootDir, "scripts", "runtime"));
  return rootDir;
}

function loadSpecs(rootDir) {
  return {
    skills: loadYamlFile(path.join(rootDir, "workflow", "skills.spec.yaml")),
    outputs: loadYamlFile(path.join(rootDir, "workflow", "output-templates.spec.yaml")),
  };
}


function assertOutputTemplatesFrozen(manifest, outputSpec, gate) {
  const manifestEntry = manifest && manifest.specs ? manifest.specs["workflow/output-templates.spec.yaml"] : null;
  assert(manifestEntry, "expected output-templates entry in validate-manifest");
  assert(manifestEntry.status === "frozen", "expected manifest status frozen for output-templates");
  assert(outputSpec && outputSpec.status === "frozen", "expected output-templates spec status frozen");

  const parseGate = (value) => {
    if (typeof value !== "string") { return null; }
    const match = value.match(/^P(\d+)-N(\d+)$/i);
    if (!match) { return null; }
    return { phase: Number(match[1]), node: Number(match[2]) };
  };
  const left = parseGate(manifestEntry.freeze_gate);
  const right = parseGate(gate);
  if (left && right) {
    const cmp = left.phase !== right.phase ? left.phase - right.phase : left.node - right.node;
    assert(cmp <= 0, "expected output-templates freeze gate to be at or before " + gate);
  }
}

function buildOutputTemplateSet(templatesSpec) {
  const templates = templatesSpec && templatesSpec.templates ? templatesSpec.templates : {};
  return new Set(Object.keys(templates));
}

function collectSkillOutputTemplates(skillsSpec) {
  const defaults = skillsSpec && skillsSpec.defaults ? skillsSpec.defaults : {};
  const defaultTemplates = Array.isArray(defaults.output_templates) ? defaults.output_templates : [];
  const skills = skillsSpec && skillsSpec.skills ? skillsSpec.skills : {};
  const items = [];

  for (const [skillId, entry] of Object.entries(skills)) {
    const list = Array.isArray(entry.output_templates) ? entry.output_templates : defaultTemplates;
    items.push({
      skillId,
      templates: list,
      hasCustom: Array.isArray(entry.output_templates),
    });
  }

  return { defaultTemplates, items };
}

function assertSkillTemplateRefs(templatesSpec, skillsSpec) {
  const templateSet = buildOutputTemplateSet(templatesSpec);
  assert(templateSet.size > 0, "expected output templates registry to be non-empty");

  const { defaultTemplates, items } = collectSkillOutputTemplates(skillsSpec);
  for (const templateId of defaultTemplates) {
    assert(templateSet.has(templateId), 'default output template missing in registry: ' + templateId);
  }

  for (const item of items) {
    assert(Array.isArray(item.templates), 'expected output_templates array for ' + item.skillId);
    for (const templateId of item.templates) {
      assert(templateSet.has(templateId), item.skillId + ' output_templates references unknown template ' + templateId);
    }
  }
}

function buildDummyEntry(templateId, template) {
  const outputLevel = template.output_level || "summary";
  const variables = {};
  const required = Array.isArray(template.required_variables) ? template.required_variables : [];
  for (const key of required) {
    variables[key] = "fixture";
  }
  return {
    template_id: templateId,
    output_level: outputLevel,
    variables,
  };
}

function assertTemplateEntriesValidate(templatesSpec) {
  const templates = templatesSpec && templatesSpec.templates ? templatesSpec.templates : {};
  const entries = Object.entries(templates).map(([templateId, template]) => buildDummyEntry(templateId, template));
  const result = validateOutputEntries(entries, templates);
  assert(result.ok, 'expected output template fixtures to validate: ' + JSON.stringify(result.issues));
}

function assertOutputLogAppend(rootDir, templateId) {
  const templatesSpec = loadYamlFile(path.join(rootDir, "workflow", "output-templates.spec.yaml"));
  const template = templatesSpec.templates[templateId];
  const entry = buildDummyEntry(templateId, template);
  appendOutputLog(rootDir, entry);
  const entries = readOutputLog(rootDir);
  assert(entries.length >= 1, "expected output log entry to be appended");
  assert(entries[entries.length - 1].template_id === templateId, "expected output log to record template id");
}

function assertAdapterBundleIncludesOutputTemplates(projectRoot) {
  const bundle = compileAdapterBundle({ rootDir: projectRoot, engine: "codex" });
  const skillsPass = bundle && bundle.passes ? bundle.passes.skills : null;
  assert(skillsPass && skillsPass.output_templates, "expected adapter bundle skills pass to include output_templates");
  const keys = Object.keys(skillsPass.output_templates);
  assert(keys.length > 0, "expected adapter bundle output_templates to be non-empty");
}

const cases = {
  "hook-client-output-log": () => {
    const rootDir = createFixtureRoot();
    const templateId = "status-indicator";
    const templatesSpec = loadYamlFile(path.join(rootDir, "workflow", "output-templates.spec.yaml"));
    const template = templatesSpec.templates[templateId];
    const entry = buildDummyEntry(templateId, template);
    persistOutputTemplatesForTest(rootDir, [entry]);
    const entries = readOutputLog(rootDir);
    assert(entries.length >= 1, "expected output log entry to be appended by hook-client");
    assert(entries[entries.length - 1].template_id === templateId, "expected hook-client to log template id");
  },
  "output-templates-freeze-gate": () => {
    const rootDir = createFixtureRoot();
    const manifest = loadYamlFile(path.join(rootDir, "workflow", "validate-manifest.yaml"));
    const outputSpec = loadYamlFile(path.join(rootDir, "workflow", "output-templates.spec.yaml"));
    assertOutputTemplatesFrozen(manifest, outputSpec, "P4-N1");
  },
  "skills-output-template-refs": () => {
    const rootDir = createFixtureRoot();
    const { skills, outputs } = loadSpecs(rootDir);
    assertSkillTemplateRefs(outputs, skills);
  },
  "output-templates-validate": () => {
    const rootDir = createFixtureRoot();
    const { outputs } = loadSpecs(rootDir);
    assertTemplateEntriesValidate(outputs);
  },
  "output-log-append": () => {
    const rootDir = createFixtureRoot();
    const templateId = "status-indicator";
    assertOutputLogAppend(rootDir, templateId);
  },
  "adapter-bundle-output-templates": () => {
    assertAdapterBundleIncludesOutputTemplates(process.cwd());
  },
};

let failed = false;
for (const [name, run] of Object.entries(cases)) {
  try {
    run();
    console.log("CASE_PASS " + name);
  } catch (error) {
    failed = true;
    console.error("CASE_FAIL " + name);
    console.error(error.stack || error.message);
  }
}

if (failed) {
  process.exit(1);
}

console.log("OUTPUT_CONTRACT_PASS");
