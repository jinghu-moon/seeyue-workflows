#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { compileAdapterBundle } = require("./compile-adapter.cjs");
const {
  attachGeneratedMetadata,
  buildGeneratedMetadata,
  wrapGeneratedSection,
} = require("./adapter-utils.cjs");
const { buildSkillsManifest } = require("../runtime/skills-manifest.cjs");

const SOURCE_OF_TRUTH_FILES = [
  "workflow/runtime.schema.yaml",
  "workflow/router.spec.yaml",
  "workflow/policy.spec.yaml",
  "workflow/capabilities.yaml",
  "workflow/persona-bindings.yaml",
  "workflow/file-classes.yaml",
  "workflow/approval-matrix.yaml",
  "workflow/hooks.spec.yaml",
  "docs/architecture-v4.md",
];

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function resolveRootDir(options = {}, bundle = null) {
  if (options.rootDir) {
    return path.resolve(options.rootDir);
  }
  if (bundle && bundle.generated_from_root) {
    return path.resolve(bundle.generated_from_root);
  }
  return path.resolve(__dirname, "..", "..");
}

function getBundle(options = {}) {
  if (options.bundle) {
    return options.bundle;
  }
  return compileAdapterBundle({ rootDir: options.rootDir, engine: "codex" });
}

function assertCodexBundle(bundle) {
  if (!bundle || bundle.engine !== "codex") {
    throw new Error(`CODEX_ADAPTER_ENGINE_MISMATCH expected=codex actual=${JSON.stringify(bundle && bundle.engine)}`);
  }
}

function walkSkillFiles(skillsRoot) {
  if (!fs.existsSync(skillsRoot)) {
    return [];
  }

  const entries = [];
  const stack = [skillsRoot];
  while (stack.length > 0) {
    const currentDir = stack.pop();
    const children = fs.readdirSync(currentDir, { withFileTypes: true });
    for (const child of children) {
      const childPath = path.join(currentDir, child.name);
      if (child.isDirectory()) {
        stack.push(childPath);
        continue;
      }
      if (child.isFile() && child.name === "SKILL.md") {
        entries.push(childPath);
      }
    }
  }

  return entries.sort((left, right) => normalizePath(left).localeCompare(normalizePath(right)));
}

function parseFrontmatter(markdownText) {
  const normalized = String(markdownText || "");
  const match = normalized.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?/);
  if (!match) {
    return {};
  }

  const frontmatter = {};
  let activeListKey = null;
  for (const line of match[1].split(/\r?\n/)) {
    const listMatch = line.match(/^\s+-\s+(.*)$/);
    if (listMatch && activeListKey) {
      if (!Array.isArray(frontmatter[activeListKey])) {
        frontmatter[activeListKey] = [];
      }
      frontmatter[activeListKey].push(listMatch[1].trim());
      continue;
    }

    const separatorIndex = line.indexOf(":");
    if (separatorIndex <= 0) {
      activeListKey = null;
      continue;
    }

    const key = line.slice(0, separatorIndex).trim();
    const value = line.slice(separatorIndex + 1).trim();
    if (key) {
      if (value.length === 0) {
        frontmatter[key] = [];
        activeListKey = key;
        continue;
      }
      frontmatter[key] = value;
      activeListKey = null;
    }
  }
  return frontmatter;
}

function buildSkillMetadata(bundle, options = {}) {
  assertCodexBundle(bundle);
  const rootDir = resolveRootDir(options, bundle);
  const skillsRoot = path.join(rootDir, bundle.render_targets.skill_root);
  const skillFiles = walkSkillFiles(skillsRoot);
  if (skillFiles.length === 0) {
    throw new Error(`CODEX_ADAPTER_MISSING_SKILL_METADATA skill_root=${normalizePath(path.relative(rootDir, skillsRoot))}`);
  }

  const skills = skillFiles.map((skillFile) => {
    const raw = fs.readFileSync(skillFile, "utf8");
    const frontmatter = parseFrontmatter(raw);
    if (!frontmatter.name || !frontmatter.description) {
      throw new Error(`CODEX_ADAPTER_MISSING_SKILL_METADATA skill_file=${normalizePath(path.relative(rootDir, skillFile))}`);
    }
    const disableModelInvocation = String(frontmatter["disable-model-invocation"] || "").trim().toLowerCase() === "true";
    const argumentHint = typeof frontmatter["argument-hint"] === "string" ? frontmatter["argument-hint"].trim() : "";
    const relativeSkillDir = normalizePath(path.relative(rootDir, path.dirname(skillFile)));
    return {
      name: frontmatter.name,
      description: frontmatter.description,
      relative_path: relativeSkillDir,
      skill_file: `${relativeSkillDir}/SKILL.md`,
      allowed_tools: Array.isArray(frontmatter["allowed-tools"])
        ? frontmatter["allowed-tools"]
        : [],
      argument_hint: argumentHint || null,
      disable_model_invocation: disableModelInvocation,
    };
  });

  return {
    schema_kind: "codex_skill_metadata",
    schema_version: 1,
    schema_dialect: "seeyue_workflow_codex_adapter/v1",
    generated_from_root: normalizePath(rootDir),
    skill_root: bundle.render_targets.skill_root,
    progressive_disclosure: true,
    source_of_truth: [".agents/skills", ...SOURCE_OF_TRUTH_FILES],
    skills,
  };
}

function buildCodexConfig(bundle) {
  assertCodexBundle(bundle);
  return [
    '# Generated artifact for `codex`. Do not edit manually.',
    '# Source of truth lives under `workflow/*.yaml` and `.agents/skills`.',
    '',
    'approval_policy = "on-request"',
    'sandbox_mode = "workspace-write"',
    '',
    '[history]',
    'persistence = "save-all"',
    '',
    '[features]',
    'child_agents_md = true',
    'web_search_request = true',
    '',
  ].join("\n");
}

function buildCodexInstructions(bundle, skillMetadata) {
  assertCodexBundle(bundle);
  const reviewChain = bundle.review_chain.map((entry) => `\`${entry}\``).join(" -> ");
  const sourceOfTruth = SOURCE_OF_TRUTH_FILES.map((entry) => `- \`${entry}\``);
  const coreSkills = skillMetadata.skills
    .filter((entry) => ["sy-workflow", "sy-constraints", "sy-executing-plans"].includes(entry.name))
    .map((entry) => `\`${entry.name}\``)
    .join(", ");

  const lines = [
    '# AGENTS.md',
    '',
    '> Generated artifact for `codex`. Do not edit manually.',
    '> Vendor files are deployment artifacts. `workflow/*.yaml` remains the machine source of truth.',
    '',
    '## Scope And Layering',
    '',
    '- This file defines the root instruction layer for Codex.',
    '- With `features.child_agents_md = true`, nested `AGENTS.md` files may add narrower instructions by directory scope.',
    '- Prefer durable workflow state under `.ai/workflow/` over chat memory or free-form recap.',
    '',
    '## Language Policy',
    '',
    '- Write machine-facing rules, contracts, plans, and skill logic in English.',
    '- Write human-facing approvals, blockers, and status updates in concise zh-CN.',
    '- Keep approval text short, explicit, and action-oriented.',
    '- Human-facing approval requests MUST use runtime-approved zh-CN short actionable copy.',
    '- Human-facing manual restore blockers MUST use runtime-approved zh-CN short actionable copy.',
    '',
    '## Source Of Truth',
    '',
    ...sourceOfTruth,
    '- `.codex/skill-metadata.json`',
    '',
    '## Routing Summary',
    '',
    '- Execution is state-first and blocker-first.',
    '- V4 Phase 1 uses a single active phase and a single active node.',
    `- Default review chain: ${reviewChain}.`,
    '- `recommended_next` and `restore_reason` MUST come from runtime state, not free-form chat reasoning.',
    '',
    '## Approval And Sandbox',
    '',
    '- Use `approval_policy = "on-request"` and `sandbox_mode = "workspace-write"` as the minimum safe Codex profile.',
    '- Destructive, git-mutating, privileged, schema-mutating, data-mutating, and sensitive network actions require human approval.',
    '- Notify-only relief is limited to low-risk `docs`, `scaffold`, and `utility` changes after verification passes.',
    '- If runtime enters `approval_pending`, surface the runtime approval request in zh-CN short actionable copy and wait.',
    '',
    '## Recovery And Resume',
    '',
    '- If runtime enters `restore_pending`, resolve recovery before any new write or command.',
    '- If manual intervention is required, surface the runtime restore request in zh-CN short actionable copy and stop.',
    '- Treat runtime recovery state as authoritative over chat recap when the two diverge.',
    '',
    '## Skills',
    '',
    '- Skill discovery metadata is compiled into `.codex/skill-metadata.json`.',
    '- Load skills with progressive disclosure only: inspect metadata first, then open the exact `SKILL.md` required for the active task.',
    `- Core workflow skills in this repository include: ${coreSkills || '`sy-workflow`'}.`,
    '- Keep reviewer personas isolated from author context when invoking workflow or review skills.',
    '',
    '## Skill Frontmatter',
    '',
    '- `$ARGUMENTS` captures the command tail; `$0`, `$1`, ... map positional arguments.',
    '- `disable-model-invocation: true` marks manual-only skills or commands.',
    '- Honor `allowed-tools` as the maximum tool scope for a skill.',
  ];

  return `${lines.join("\n")}\n`;
}

function renderCodexArtifacts(options = {}) {
  const bundle = getBundle(options);
  const skillMetadata = buildSkillMetadata(bundle, options);
  const instructionFile = wrapGeneratedSection(
    buildCodexInstructions(bundle, skillMetadata),
    buildGeneratedMetadata(bundle, "routing"),
    "markdown",
  );
  const configToml = wrapGeneratedSection(
    buildCodexConfig(bundle),
    buildGeneratedMetadata(bundle, "policy"),
    "toml",
  );
  const skillMetadataWithMeta = attachGeneratedMetadata(
    skillMetadata,
    buildGeneratedMetadata(bundle, "skills"),
  );
  const capabilityGap = attachGeneratedMetadata(
    bundle.capability_gap_report,
    buildGeneratedMetadata(bundle, "policy", { artifact: "capability-gap" }),
  );
  const baseFiles = {
    "AGENTS.md": instructionFile,
    ".codex/config.toml": `${configToml}\n`,
    ".codex/skill-metadata.json": `${JSON.stringify(skillMetadataWithMeta, null, 2)}\n`,
    ".ai/workflow/capability-gap.json": `${JSON.stringify(capabilityGap, null, 2)}\n`,
  };
  const manifestFiles = [...Object.keys(baseFiles), ".ai/workflow/skills-manifest.json"];
  const skillsManifest = attachGeneratedMetadata(
    buildSkillsManifest({ bundle, generated_files: manifestFiles }),
    buildGeneratedMetadata(bundle, "skills", { artifact: "skills-manifest" }),
  );

  return {
    engine: "codex",
    bundle,
    skillMetadata: skillMetadataWithMeta,
    files: {
      ...baseFiles,
      ".ai/workflow/skills-manifest.json": `${JSON.stringify(skillsManifest, null, 2)}\n`,
    },
  };
}

function writeCodexArtifacts(options = {}) {
  const rendered = renderCodexArtifacts(options);
  const outputRootDir = path.resolve(options.outputRootDir || resolveRootDir(options, rendered.bundle));
  const writtenFiles = [];

  for (const [relativePath, content] of Object.entries(rendered.files)) {
    const targetPath = path.join(outputRootDir, relativePath);
    ensureDir(path.dirname(targetPath));
    fs.writeFileSync(targetPath, content, "utf8");
    writtenFiles.push(normalizePath(path.relative(outputRootDir, targetPath)));
  }

  return {
    ...rendered,
    output_root: normalizePath(outputRootDir),
    written_files: writtenFiles,
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    outputRootDir: null,
    write: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--output":
        index += 1;
        parsed.outputRootDir = path.resolve(argv[index]);
        break;
      case "--write":
        parsed.write = true;
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
    if (parsed.write) {
      const result = writeCodexArtifacts(parsed);
      console.log("CODEX_ARTIFACTS_WRITTEN");
      for (const filePath of result.written_files) {
        console.log(filePath);
      }
      return;
    }

    const rendered = renderCodexArtifacts(parsed);
    console.log(JSON.stringify({ engine: rendered.engine, files: Object.keys(rendered.files) }, null, 2));
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  buildCodexConfig,
  buildCodexInstructions,
  buildSkillMetadata,
  renderCodexArtifacts,
  writeCodexArtifacts,
};

if (require.main === module) {
  main();
}
