"use strict";

const path = require("node:path");
const { loadYamlFile } = require("./yaml-loader.cjs");

const SPEC_FILES = {
  capabilities: "workflow/capabilities.yaml",
  personaBindings: "workflow/persona-bindings.yaml",
  fileClasses: "workflow/file-classes.yaml",
  approvalMatrix: "workflow/approval-matrix.yaml",
  hookSpec: "workflow/hooks.spec.yaml",
  hookContract: "workflow/hook-contract.schema.yaml",
  skillsSpec: "workflow/skills.spec.yaml",
  outputTemplates: "workflow/output-templates.spec.yaml",
  validateManifest: "workflow/validate-manifest.yaml",
  runtimeSchema: "workflow/runtime.schema.yaml",
  routerSpec: "workflow/router.spec.yaml",
  policySpec: "workflow/policy.spec.yaml",
};

function loadWorkflowSpecs(rootDir) {
  const baseDir = path.resolve(rootDir || path.join(__dirname, "..", ".."));
  const loaded = {};
  for (const [key, relativePath] of Object.entries(SPEC_FILES)) {
    loaded[key] = loadYamlFile(path.join(baseDir, relativePath));
  }
  return loaded;
}

module.exports = {
  SPEC_FILES,
  loadWorkflowSpecs,
};
