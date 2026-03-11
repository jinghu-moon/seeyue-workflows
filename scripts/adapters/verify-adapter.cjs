#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { renderClaudeCodeArtifacts } = require("./claude-code.cjs");
const { renderCodexArtifacts } = require("./codex.cjs");
const { renderGeminiArtifacts } = require("./gemini-cli.cjs");
const { stripSeededSections } = require("./adapter-utils.cjs");

const VOLATILE_KEYS = new Set(["generated_at", "updated_at"]);

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function isObject(value) {
  return value && typeof value === "object" && !Array.isArray(value);
}

function stripVolatileFields(value) {
  if (Array.isArray(value)) {
    return value.map((entry) => stripVolatileFields(entry));
  }
  if (!isObject(value)) {
    return value;
  }
  const result = {};
  for (const [key, entry] of Object.entries(value)) {
    if (VOLATILE_KEYS.has(key)) {
      continue;
    }
    result[key] = stripVolatileFields(entry);
  }
  return result;
}

function ensureGeneratedMarkers(text, format) {
  if (format === "toml") {
    return /#\s*SY:GENERATED:BEGIN/.test(text) && /#\s*SY:GENERATED:END/.test(text);
  }
  return /<!--\s*SY:GENERATED:BEGIN/.test(text) && /<!--\s*SY:GENERATED:END\s*-->/.test(text);
}

function normalizeText(text) {
  return stripSeededSections(text).trimEnd();
}

function readFileSafely(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch {
    return null;
  }
}

function compareText(expected, actual, format) {
  if (!ensureGeneratedMarkers(actual || "", format)) {
    return { ok: false, reason: "GENERATED_MARKER_MISSING" };
  }
  if (!ensureGeneratedMarkers(expected || "", format)) {
    return { ok: false, reason: "EXPECTED_MARKER_MISSING" };
  }
  const normalizedExpected = normalizeText(expected);
  const normalizedActual = normalizeText(actual);
  return {
    ok: normalizedExpected === normalizedActual,
    reason: normalizedExpected === normalizedActual ? null : "CONTENT_MISMATCH",
  };
}

function compareJson(expectedText, actualText) {
  let expected;
  let actual;
  try {
    expected = JSON.parse(expectedText || "{}");
    actual = JSON.parse(actualText || "{}");
  } catch (error) {
    return { ok: false, reason: `JSON_PARSE_FAIL ${error.message}` };
  }
  if (!isObject(actual) || !isObject(actual._sy_generated)) {
    return { ok: false, reason: "GENERATED_METADATA_MISSING" };
  }
  const normalizedExpected = stripVolatileFields(expected);
  const normalizedActual = stripVolatileFields(actual);
  return {
    ok: JSON.stringify(normalizedExpected) === JSON.stringify(normalizedActual),
    reason: JSON.stringify(normalizedExpected) === JSON.stringify(normalizedActual) ? null : "JSON_MISMATCH",
  };
}

function detectFormat(filePath) {
  if (filePath.endsWith(".md")) {
    return "markdown";
  }
  if (filePath.endsWith(".toml")) {
    return "toml";
  }
  if (filePath.endsWith(".json")) {
    return "json";
  }
  return "text";
}

function verifyArtifacts(rendered, rootDir) {
  const issues = [];
  for (const [relativePath, expectedContent] of Object.entries(rendered.files)) {
    const targetPath = path.join(rootDir, relativePath);
    const actualContent = readFileSafely(targetPath);
    if (actualContent === null) {
      issues.push({
        file: normalizePath(relativePath),
        reason: "FILE_MISSING",
      });
      continue;
    }

    const format = detectFormat(relativePath);
    const result =
      format === "json"
        ? compareJson(expectedContent, actualContent)
        : compareText(expectedContent, actualContent, format === "toml" ? "toml" : "markdown");

    if (!result.ok) {
      issues.push({
        file: normalizePath(relativePath),
        reason: result.reason || "MISMATCH",
      });
    }
  }
  return {
    ok: issues.length === 0,
    issues,
  };
}

function getRenderedArtifacts(engine, rootDir) {
  if (engine === "claude_code") {
    return renderClaudeCodeArtifacts({ rootDir });
  }
  if (engine === "codex") {
    return renderCodexArtifacts({ rootDir });
  }
  if (engine === "gemini_cli") {
    return renderGeminiArtifacts({ rootDir });
  }
  throw new Error(`UNSUPPORTED_ENGINE ${engine}`);
}

function parseArgs(argv) {
  const parsed = {
    engine: null,
    rootDir: path.resolve(__dirname, "..", ".."),
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--engine") {
      index += 1;
      parsed.engine = argv[index];
      continue;
    }
    if (token === "--root") {
      index += 1;
      parsed.rootDir = path.resolve(argv[index]);
      continue;
    }
    throw new Error(`Unknown argument: ${token}`);
  }
  if (!parsed.engine) {
    throw new Error("Missing required argument: --engine");
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
    const rendered = getRenderedArtifacts(parsed.engine, parsed.rootDir);
    const result = verifyArtifacts(rendered, parsed.rootDir);
    if (!result.ok) {
      console.error(`ADAPTER_VERIFY_FAIL engine=${parsed.engine}`);
      for (const issue of result.issues) {
        console.error(`- ${issue.file}: ${issue.reason}`);
      }
      process.exit(1);
    }
    console.log(`ADAPTER_VERIFY_PASS engine=${parsed.engine}`);
  } catch (error) {
    console.error(`ADAPTER_VERIFY_FAIL ${error.message}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  verifyArtifacts,
};
