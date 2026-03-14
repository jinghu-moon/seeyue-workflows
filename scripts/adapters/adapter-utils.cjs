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

const SEEDED_MARKERS = {
  markdown: {
    begin: "<!-- SY:SEEDED:BEGIN -->",
    end: "<!-- SY:SEEDED:END -->",
    placeholder: "<!-- SY:SEEDED:OMITTED -->",
  },
  toml: {
    begin: "# SY:SEEDED:BEGIN",
    end: "# SY:SEEDED:END",
    placeholder: "# SY:SEEDED:OMITTED",
  },
};

function buildSeededSection(format) {
  const markers = SEEDED_MARKERS[format];
  if (!markers) {
    return "";
  }
  const hint = format === "toml"
    ? "# Add project-specific overrides below."
    : "<!-- Add project-specific overrides below. -->";
  return [markers.begin, hint, markers.end].join("\n");
}

function extractSeededSection(text, format) {
  const input = String(text || "");
  if (format === "toml") {
    const match = input.match(/^#\s*SY:SEEDED:BEGIN[\s\S]*?^#\s*SY:SEEDED:END.*$/gmi);
    return match ? match[0] : null;
  }
  const match = input.match(/<!--\s*SY:SEEDED:BEGIN[\s\S]*?SY:SEEDED:END\s*-->/i);
  return match ? match[0] : null;
}

function hasSeededMarkers(text, format) {
  const markers = SEEDED_MARKERS[format];
  if (!markers) {
    return false;
  }
  return String(text || "").includes(markers.begin) && String(text || "").includes(markers.end);
}

function mergeSeededSections(existingText, nextText, format) {
  if (!existingText) {
    return nextText;
  }
  const existingSeeded = extractSeededSection(existingText, format);
  if (!existingSeeded) {
    return nextText;
  }
  const seededPattern = format === "toml"
    ? /^#\s*SY:SEEDED:BEGIN[\s\S]*?^#\s*SY:SEEDED:END.*$/gmi
    : /<!--\s*SY:SEEDED:BEGIN[\s\S]*?SY:SEEDED:END\s*-->/gi;
  if (extractSeededSection(nextText, format)) {
    return String(nextText || "").replace(seededPattern, existingSeeded);
  }
  return `${String(nextText || "").trimEnd()}\n${existingSeeded}\n`;
}

function detectSeededFormat(filePath) {
  const normalized = String(filePath || "");
  if (normalized.endsWith(".md")) {
    return "markdown";
  }
  if (normalized.endsWith(".toml")) {
    return "toml";
  }
  return null;
}

function wrapGeneratedSection(content, metadata, format = "markdown") {
  const payload = JSON.stringify(metadata || {});
  const trimmed = String(content || "").trimEnd();
  if (format === "toml") {
    const generated = [
      `# SY:GENERATED:BEGIN ${payload}`,
      trimmed,
      "# SY:GENERATED:END",
      "",
    ].join("\n");
    const seeded = buildSeededSection("toml");
    return seeded ? `${generated}${seeded}\n` : generated;
  }
  const generated = [
    `<!-- SY:GENERATED:BEGIN ${payload} -->`,
    trimmed,
    "<!-- SY:GENERATED:END -->",
    "",
  ].join("\n");
  const seeded = buildSeededSection("markdown");
  return seeded ? `${generated}${seeded}\n` : generated;
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
  return input
    .replace(htmlSeeded, SEEDED_MARKERS.markdown.placeholder)
    .replace(tomlSeeded, SEEDED_MARKERS.toml.placeholder);
}

module.exports = {
  attachGeneratedMetadata,
  buildGeneratedMetadata,
  detectSeededFormat,
  extractSeededSection,
  hasSeededMarkers,
  mergeSeededSections,
  normalizePath,
  SEEDED_MARKERS,
  stripSeededSections,
  wrapGeneratedSection,
};
