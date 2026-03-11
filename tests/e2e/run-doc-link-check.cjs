#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const projectRoot = path.resolve(__dirname, "..", "..");

const REQUIRED_DOCS = {
  "docs/adoption-guide.md": {
    headings: ["## 目标", "## 前置条件", "## 接入步骤", "## 验证重点", "## 常见问题"],
    mustLink: [
      "scripts/sync-workflow-assets.py",
      "sync-manifest.json",
      "workflow/runtime.schema.yaml",
      "workflow/router.spec.yaml",
      "workflow/policy.spec.yaml",
      "tests/e2e/run-engine-conformance.cjs",
      "tests/e2e/run-doc-link-check.cjs",
      "docs/operations-runbook.md",
      "docs/versioning-policy.md",
    ],
  },
  "docs/release-checklist.md": {
    headings: ["## 目标", "## 发布前", "## 同步前", "## 发布后", "## 回滚"],
    mustLink: [
      "CHANGELOG.md",
      "sync-manifest.json",
      "docs/source-of-truth.md",
      "docs/operations-runbook.md",
      "docs/versioning-policy.md",
    ],
  },
  "docs/source-of-truth.md": {
    headings: ["## 目标", "## 机器事实源", "## 运行态事实源", "## 分发入口", "## 版本与同步", "## 阅读顺序"],
    mustLink: [
      "workflow/runtime.schema.yaml",
      "workflow/router.spec.yaml",
      "workflow/policy.spec.yaml",
      "scripts/adapters/claude-code.cjs",
      "scripts/adapters/codex.cjs",
      "scripts/adapters/gemini-cli.cjs",
      "sync-manifest.json",
      "docs/versioning-policy.md",
    ],
  },
  "docs/operations-runbook.md": {
    headings: ["## 目标", "## 日常巡检", "## 运行态恢复", "## 审计与证据", "## 变更与发布", "## 常见故障"],
    mustLink: [
      "scripts/runtime/validate-specs.cjs",
      "scripts/runtime/recovery-bridge.cjs",
      "scripts/runtime/context-manager.cjs",
      "scripts/runtime/journal.cjs",
      "scripts/hooks/sy-pretool-write.cjs",
      "tests/e2e/run-engine-conformance.cjs",
      "docs/versioning-policy.md",
      "docs/release-checklist.md",
    ],
  },
  "docs/README.md": {
    headings: ["## 核心文档", "## 实施材料", "## 规范草案", "## 机器事实源"],
    mustLink: [
      "docs/adoption-guide.md",
      "docs/operations-runbook.md",
      "docs/release-checklist.md",
      "docs/source-of-truth.md",
      "docs/versioning-policy.md",
      "workflow/runtime.schema.yaml",
      "workflow/router.spec.yaml",
      "workflow/policy.spec.yaml",
    ],
  },
  "docs/versioning-policy.md": {
    headings: ["## 目标", "## Version Model", "## Semver Rules", "## Adapter Version Rules", "## Breaking Change Contract", "## Sync Contract", "## Release Order", "## Source Files"],
    mustLink: [
      "package.json",
      "CHANGELOG.md",
      "sync-manifest.json",
      "docs/release-checklist.md",
      "docs/source-of-truth.md",
    ],
  },
};

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function normalize(value) {
  return String(value || "").replace(/\\/g, "/");
}

function readFile(relativePath) {
  const absolutePath = path.join(projectRoot, relativePath);
  assert(fs.existsSync(absolutePath), `DOC_FILE_MISSING path=${relativePath}`);
  return fs.readFileSync(absolutePath, "utf8");
}

function extractMarkdownLinks(markdownText) {
  const links = [];
  const regex = /?\[[^\]]*\]\(([^)]+)\)/g;
  let match;
  while ((match = regex.exec(markdownText)) !== null) {
    links.push(match[1].trim());
  }
  return links;
}

function resolveDocLink(docPath, rawLink) {
  if (!rawLink || rawLink.startsWith("#") || /^https?:\/\//i.test(rawLink) || /^mailto:/i.test(rawLink)) {
    return null;
  }
  const withoutAnchor = rawLink.split("#")[0];
  if (!withoutAnchor) {
    return null;
  }
  const baseDir = path.dirname(path.join(projectRoot, docPath));
  return normalize(path.relative(projectRoot, path.resolve(baseDir, withoutAnchor)));
}

function checkDoc(relativePath, spec) {
  const markdown = readFile(relativePath);

  for (const heading of spec.headings) {
    assert(markdown.includes(heading), `DOC_HEADING_MISSING path=${relativePath} heading=${JSON.stringify(heading)}`);
  }

  const resolvedLinks = extractMarkdownLinks(markdown)
    .map((rawLink) => resolveDocLink(relativePath, rawLink))
    .filter(Boolean);

  for (const target of resolvedLinks) {
    const absoluteTarget = path.join(projectRoot, target);
    assert(fs.existsSync(absoluteTarget), `DOC_LINK_BROKEN path=${relativePath} target=${target}`);
  }

  for (const target of spec.mustLink) {
    assert(resolvedLinks.includes(normalize(target)), `DOC_LINK_REQUIRED_MISSING path=${relativePath} target=${target}`);
  }
}

function main() {
  try {
    for (const [relativePath, spec] of Object.entries(REQUIRED_DOCS)) {
      checkDoc(relativePath, spec);
      console.log(`DOC_PASS ${relativePath}`);
    }
    console.log("DOC_LINK_CHECK_PASS");
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

main();
