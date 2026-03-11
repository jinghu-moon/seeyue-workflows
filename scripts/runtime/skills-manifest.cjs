"use strict";

const path = require("node:path");

const { loadWorkflowSpecs } = require("./workflow-specs.cjs");
const { buildSkillRegistry } = require("../adapters/compile-adapter.cjs");

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function uniq(list) {
  return Array.from(new Set((Array.isArray(list) ? list : []).filter(Boolean)));
}

function resolveRootDir(rootDir) {
  return path.resolve(rootDir || path.join(__dirname, "..", ".."));
}

function getRegistry(bundle, rootDir) {
  if (bundle?.passes?.skills?.skill_registry) {
    return bundle.passes.skills.skill_registry;
  }
  const specs = loadWorkflowSpecs(resolveRootDir(rootDir));
  return buildSkillRegistry(specs);
}

function buildSkillsManifest(options = {}) {
  const rootDir = resolveRootDir(options.rootDir);
  const bundle = options.bundle || null;
  const registry = getRegistry(bundle, rootDir);
  const generatedFiles = uniq((options.generated_files || options.generatedFiles || []).map(normalizePath));
  const skillRoot = normalizePath(bundle?.render_targets?.skill_root || ".agents/skills");
  const skills = Array.isArray(registry?.skills) ? registry.skills : [];

  return {
    schema_kind: "skills_manifest",
    schema_version: 1,
    schema_dialect: "seeyue_workflow_manifest/v1",
    generated_at: new Date().toISOString(),
    engine: bundle?.engine || null,
    registry_revision: registry?.registry_revision || "",
    spec_hash: registry?.spec_hash || "",
    change_detection: registry?.change_detection || {},
    skill_root: skillRoot,
    skill_count: skills.length,
    source_of_truth: ["workflow/skills.spec.yaml", skillRoot],
    generated_files: generatedFiles,
  };
}

module.exports = {
  buildSkillsManifest,
};
