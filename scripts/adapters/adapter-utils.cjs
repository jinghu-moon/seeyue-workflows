"use strict";

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function buildGeneratedMetadata(bundle, pass, extra = {}) {
  const skillRegistry = bundle?.passes?.skills?.skill_registry || {};
  return {
    generator: bundle?.compiler?.name || "seeyue-compile-adapter",
    version: bundle?.compiler?.version ?? 1,
    engine: bundle?.engine || "unknown",
    pass,
    registry_revision: skillRegistry.registry_revision || null,
    spec_hash: skillRegistry.spec_hash || null,
    ...extra,
  };
}

function wrapGeneratedSection(content, metadata, format = "markdown") {
  const payload = JSON.stringify(metadata || {});
  const trimmed = String(content || "").trimEnd();
  if (format === "toml") {
    return [
      `# SY:GENERATED:BEGIN ${payload}`,
      trimmed,
      "# SY:GENERATED:END",
      "",
    ].join("\n");
  }
  return [
    `<!-- SY:GENERATED:BEGIN ${payload} -->`,
    trimmed,
    "<!-- SY:GENERATED:END -->",
    "",
  ].join("\n");
}

function attachGeneratedMetadata(obj, metadata) {
  const base = obj && typeof obj === "object" ? obj : {};
  return {
    ...base,
    _sy_generated: metadata || {},
  };
}

function stripSeededSections(text) {
  const input = String(text || "");
  const htmlSeeded = /<!--\s*SY:SEEDED:BEGIN[\s\S]*?SY:SEEDED:END\s*-->/gi;
  const tomlSeeded = /^#\s*SY:SEEDED:BEGIN[\s\S]*?^#\s*SY:SEEDED:END.*$/gmi;
  return input.replace(htmlSeeded, "<!-- SY:SEEDED:OMITTED -->").replace(tomlSeeded, "# SY:SEEDED:OMITTED");
}

module.exports = {
  attachGeneratedMetadata,
  buildGeneratedMetadata,
  normalizePath,
  stripSeededSections,
  wrapGeneratedSection,
};
