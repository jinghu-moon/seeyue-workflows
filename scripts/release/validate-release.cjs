#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const SEMVER_RE = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;
const CHANGELOG_RE = /^##\s+([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?)\s+-\s+(\d{4}-\d{2}-\d{2})/m;
const REQUIRED_SYNC_DOCS = [
  ["docs/adoption-guide.md", "docs/workflow/adoption-guide.md"],
  ["docs/source-of-truth.md", "docs/workflow/source-of-truth.md"],
  ["docs/release-checklist.md", "docs/workflow/release-checklist.md"],
  ["docs/operations-runbook.md", "docs/workflow/operations-runbook.md"],
  ["docs/versioning-policy.md", "docs/workflow/versioning-policy.md"],
];
const REQUIRED_PACKAGE_SCRIPTS = [
  "test:e2e:release",
  "test:e2e:engine-conformance",
  "test:e2e:doc-link-check",
];
const REQUIRED_ADAPTERS = ["claude_code", "codex", "gemini_cli"];

function assert(condition, code, detail) {
  if (!condition) {
    throw new Error(detail ? `${code} ${detail}` : code);
  }
}

function readJson(rootDir, relativePath) {
  const absolutePath = path.join(rootDir, relativePath);
  assert(fs.existsSync(absolutePath), "RELEASE_FILE_MISSING", `path=${relativePath}`);
  return JSON.parse(fs.readFileSync(absolutePath, "utf8"));
}

function readText(rootDir, relativePath) {
  const absolutePath = path.join(rootDir, relativePath);
  assert(fs.existsSync(absolutePath), "RELEASE_FILE_MISSING", `path=${relativePath}`);
  return fs.readFileSync(absolutePath, "utf8");
}

function validateSemver(value, code, fieldName) {
  assert(typeof value === "string" && SEMVER_RE.test(value), code, `field=${fieldName} actual=${JSON.stringify(value)}`);
}

function extractLatestRelease(changelogText) {
  const match = String(changelogText || "").match(CHANGELOG_RE);
  assert(Boolean(match), "RELEASE_CHANGELOG_HEADING_MISSING", "expected heading like `## 0.2.0 - 2026-03-09`");
  return {
    version: match[1],
    releaseDate: match[2],
  };
}

function hasSyncEntry(manifest, source, target) {
  const entries = Array.isArray(manifest?.entries) ? manifest.entries : [];
  return entries.some((entry) => entry && entry.source === source && entry.target === target);
}

function validatePackageScripts(packageJson) {
  const scripts = packageJson?.scripts || {};
  for (const scriptName of REQUIRED_PACKAGE_SCRIPTS) {
    assert(typeof scripts[scriptName] === "string" && scripts[scriptName].trim().length > 0, "RELEASE_SCRIPT_MISSING", `script=${scriptName}`);
  }
}

function validateAdapterVersions(adapterVersions) {
  assert(adapterVersions && typeof adapterVersions === "object" && !Array.isArray(adapterVersions), "RELEASE_ADAPTER_VERSIONS_MISSING");
  for (const engine of REQUIRED_ADAPTERS) {
    const version = adapterVersions[engine];
    assert(Number.isInteger(version) && version >= 1, "RELEASE_ADAPTER_VERSION_INVALID", `engine=${engine} actual=${JSON.stringify(version)}`);
  }
}

function validateSyncEntries(manifest) {
  for (const [source, target] of REQUIRED_SYNC_DOCS) {
    assert(hasSyncEntry(manifest, source, target), "RELEASE_SYNC_ENTRY_MISSING", `source=${source} target=${target}`);
  }
}

function validateVersioningPolicy(rootDir) {
  const text = readText(rootDir, "docs/versioning-policy.md").toLowerCase();
  for (const keyword of ["workflow version", "adapter version", "breaking change", "sync-manifest"]) {
    assert(text.includes(keyword), "RELEASE_VERSIONING_DOC_INCOMPLETE", `keyword=${JSON.stringify(keyword)}`);
  }
}

function validateReleaseRoot(rootDir) {
  const absoluteRoot = path.resolve(rootDir || path.join(__dirname, "..", ".."));
  const packageJson = readJson(absoluteRoot, "package.json");
  const manifest = readJson(absoluteRoot, "sync-manifest.json");
  const changelog = readText(absoluteRoot, "CHANGELOG.md");

  validateSemver(packageJson.version, "RELEASE_PACKAGE_VERSION_INVALID", "package.version");
  validatePackageScripts(packageJson);

  assert(Number.isInteger(manifest?.schema_version) && manifest.schema_version >= 1, "RELEASE_MANIFEST_SCHEMA_INVALID");
  assert(manifest?.release && typeof manifest.release === "object" && !Array.isArray(manifest.release), "RELEASE_MANIFEST_RELEASE_MISSING");

  const latest = extractLatestRelease(changelog);
  validateSemver(manifest.release.workflow_version, "RELEASE_WORKFLOW_VERSION_INVALID", "sync-manifest.release.workflow_version");
  assert(manifest.release.workflow_version === packageJson.version, "RELEASE_MANIFEST_VERSION_DRIFT", `package=${packageJson.version} manifest=${manifest.release.workflow_version}`);
  assert(latest.version === packageJson.version, "RELEASE_CHANGELOG_VERSION_DRIFT", `package=${packageJson.version} changelog=${latest.version}`);
  assert(manifest.release.release_date === latest.releaseDate, "RELEASE_RELEASE_DATE_DRIFT", `manifest=${manifest.release.release_date} changelog=${latest.releaseDate}`);

  assert(typeof manifest.release.breaking_change === "boolean", "RELEASE_BREAKING_CHANGE_FLAG_INVALID");
  if (manifest.release.breaking_change === true) {
    const upgradeNotes = manifest.release.upgrade_notes;
    const hasNotes = (Array.isArray(upgradeNotes) && upgradeNotes.length > 0)
      || (typeof upgradeNotes === "string" && upgradeNotes.trim().length > 0);
    assert(hasNotes, "RELEASE_BREAKING_CHANGE_NOTES_MISSING");
  }

  validateAdapterVersions(manifest.release.adapter_versions);
  validateSemver(manifest.release.compatibility?.minimum_sync_version, "RELEASE_MINIMUM_SYNC_VERSION_INVALID", "sync-manifest.release.compatibility.minimum_sync_version");
  validateSyncEntries(manifest);
  validateVersioningPolicy(absoluteRoot);

  return {
    ok: true,
    workflow_version: packageJson.version,
    release_date: latest.releaseDate,
    breaking_change: manifest.release.breaking_change,
    adapter_versions: manifest.release.adapter_versions,
    sync_entries: Array.isArray(manifest.entries) ? manifest.entries.length : 0,
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
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
    const result = validateReleaseRoot(parsed.rootDir);
    console.log(JSON.stringify(result, null, 2));
    console.log("RELEASE_VALIDATION_PASS");
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  validateReleaseRoot,
};

if (require.main === module) {
  main();
}
