#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { validateReleaseRoot } = require("../../scripts/release/validate-release.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-release-fixtures-"));
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function writeText(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, value, "utf8");
}

function basePackage(version = "0.2.0") {
  return {
    name: "seeyue-workflows",
    private: true,
    version,
    scripts: {
      "test:e2e:release": "node tests/e2e/run-release-fixtures.cjs",
      "test:e2e:engine-conformance": "node tests/e2e/run-engine-conformance.cjs --all",
      "test:e2e:doc-link-check": "node tests/e2e/run-doc-link-check.cjs",
    },
  };
}

function baseManifest(workflowVersion = "0.2.0") {
  return {
    schema_version: 1,
    release: {
      workflow_version: workflowVersion,
      release_date: "2026-03-09",
      breaking_change: false,
      upgrade_notes: [],
      adapter_versions: {
        claude_code: 1,
        codex: 1,
        gemini_cli: 1,
      },
      compatibility: {
        minimum_sync_version: workflowVersion,
      },
    },
    entries: [
      {
        type: "file",
        source: "docs/adoption-guide.md",
        target: "docs/workflow/adoption-guide.md",
      },
      {
        type: "file",
        source: "docs/source-of-truth.md",
        target: "docs/workflow/source-of-truth.md",
      },
      {
        type: "file",
        source: "docs/release-checklist.md",
        target: "docs/workflow/release-checklist.md",
      },
      {
        type: "file",
        source: "docs/operations-runbook.md",
        target: "docs/workflow/operations-runbook.md",
      },
      {
        type: "file",
        source: "docs/versioning-policy.md",
        target: "docs/workflow/versioning-policy.md",
      },
    ],
  };
}

function seedMinimalReleaseRoot(rootDir, options = {}) {
  const version = options.version || "0.2.0";
  writeJson(path.join(rootDir, "package.json"), basePackage(version));

  const manifest = baseManifest(options.manifestWorkflowVersion || version);
  if (typeof options.mutateManifest === "function") {
    options.mutateManifest(manifest);
  }
  writeJson(path.join(rootDir, "sync-manifest.json"), manifest);

  const changelogDate = options.changelogDate || "2026-03-09";
  writeText(
    path.join(rootDir, "CHANGELOG.md"),
    `# Changelog\n\n## ${options.changelogVersion || version} - ${changelogDate}\n\n- Release fixture\n`,
  );

  writeText(path.join(rootDir, "docs", "release-checklist.md"), "# Release Checklist\n");
  writeText(path.join(rootDir, "docs", "adoption-guide.md"), "# Adoption Guide\n");
  writeText(path.join(rootDir, "docs", "source-of-truth.md"), "# Source of Truth\n");
  writeText(path.join(rootDir, "docs", "operations-runbook.md"), "# Operations Runbook\n");
  writeText(path.join(rootDir, "docs", "versioning-policy.md"), "# Versioning Policy\n");
}

const cases = {
  "manifest-version-drift": () => {
    const rootDir = makeTempRoot();
    seedMinimalReleaseRoot(rootDir, {
      version: "0.2.0",
      manifestWorkflowVersion: "0.2.1",
    });

    let failed = false;
    try {
      validateReleaseRoot(rootDir);
    } catch (error) {
      failed = true;
      assert(
        /RELEASE_MANIFEST_VERSION_DRIFT/i.test(String(error.message || "")),
        `expected manifest drift failure but got ${JSON.stringify(error.message)}`,
      );
    }
    assert(failed === true, "expected manifest version drift to fail");
  },
  "breaking-change-note-required": () => {
    const rootDir = makeTempRoot();
    seedMinimalReleaseRoot(rootDir, {
      mutateManifest: (manifest) => {
        manifest.release.breaking_change = true;
        manifest.release.upgrade_notes = [];
      },
    });

    let failed = false;
    try {
      validateReleaseRoot(rootDir);
    } catch (error) {
      failed = true;
      assert(
        /RELEASE_BREAKING_CHANGE_NOTES_MISSING/i.test(String(error.message || "")),
        `expected breaking change notes failure but got ${JSON.stringify(error.message)}`,
      );
    }
    assert(failed === true, "expected breaking change without notes to fail");
  },
  "sync-doc-coverage-required": () => {
    const rootDir = makeTempRoot();
    seedMinimalReleaseRoot(rootDir, {
      mutateManifest: (manifest) => {
        manifest.entries = manifest.entries.filter((entry) => entry.source !== "docs/operations-runbook.md");
      },
    });

    let failed = false;
    try {
      validateReleaseRoot(rootDir);
    } catch (error) {
      failed = true;
      assert(
        /RELEASE_SYNC_ENTRY_MISSING/i.test(String(error.message || "")),
        `expected sync entry failure but got ${JSON.stringify(error.message)}`,
      );
    }
    assert(failed === true, "expected missing sync doc entry to fail");
  },
  "release-contract-pass": () => {
    const result = validateReleaseRoot(projectRoot);
    assert(result.ok === true, `expected release validation pass but got ${JSON.stringify(result)}`);
  },
};

function parseArgs(argv) {
  const parsed = { caseName: null };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === "--case") {
      index += 1;
      parsed.caseName = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${token}`);
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

  const selected = parsed.caseName ? [[parsed.caseName, cases[parsed.caseName]]] : Object.entries(cases);
  if (selected.some(([, run]) => typeof run !== "function")) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }

  for (const [caseName, run] of selected) {
    try {
      run();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
      process.exit(1);
    }
  }

  console.log("RELEASE_FLOW_PASS");
}

main();
