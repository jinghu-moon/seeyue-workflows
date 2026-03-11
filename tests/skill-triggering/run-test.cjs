#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

function parseArgs(argv) {
  const result = {};
  for (let i = 2; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    const next = argv[i + 1];
    if (!next || next.startsWith("--")) {
      result[key] = "true";
      continue;
    }
    result[key] = next;
    i += 1;
  }
  return result;
}

function ensureDir(target) {
  fs.mkdirSync(target, { recursive: true });
}

function escapeRegex(value) {
  return String(value || "").replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function resolveProjectRoot() {
  return path.resolve(__dirname, "..", "..");
}

function normalize(value) {
  return String(value || "").trim().toLowerCase();
}

function unique(items) {
  return Array.from(new Set(items.filter(Boolean)));
}

function parseFrontmatter(markdown) {
  const text = String(markdown || "").replace(/^\uFEFF/, "");
  const m = text.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!m) return {};
  const body = String(m[1] || "");
  const out = {};
  for (const rawLine of body.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const kv = line.match(/^([A-Za-z0-9_-]+)\s*:\s*(.*)$/);
    if (!kv) continue;
    const key = String(kv[1] || "").trim();
    const value = String(kv[2] || "").trim().replace(/^['"]|['"]$/g, "");
    if (!key) continue;
    out[key] = value;
  }
  return out;
}

function loadJsonFile(filePath, fallback) {
  try {
    if (!fs.existsSync(filePath)) return fallback;
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return fallback;
  }
}

function loadPolicyKeywordRegistry(projectRoot) {
  const policyPath = path.resolve(projectRoot, ".claude", "sy-hooks.policy.json");
  const policy = loadJsonFile(policyPath, {});
  const sessionEval = policy && typeof policy === "object" ? policy.sessionEval : {};
  const expectedSkills = sessionEval && typeof sessionEval === "object" ? sessionEval.expectedSkills : [];
  const registry = {};
  if (!Array.isArray(expectedSkills)) return registry;
  for (const row of expectedSkills) {
    const entry = row && typeof row === "object" ? row : {};
    const skill = String(entry.skill || "").trim();
    const keywords = Array.isArray(entry.keywords) ? entry.keywords : [];
    if (!skill || keywords.length === 0) continue;
    registry[skill] = unique(keywords.map((k) => normalize(k)).filter(Boolean));
  }
  return registry;
}

function loadCaseKeywordRegistry(casesPath) {
  const cases = loadJsonFile(casesPath, []);
  const registry = {};
  if (!Array.isArray(cases)) return registry;
  for (const row of cases) {
    const entry = row && typeof row === "object" ? row : {};
    const skill = String(entry.skill || "").trim();
    const keywords = Array.isArray(entry.offline_keywords) ? entry.offline_keywords : [];
    if (!skill || keywords.length === 0) continue;
    registry[skill] = unique(keywords.map((k) => normalize(k)).filter(Boolean));
  }
  return registry;
}

function resolveKeywordRegistry(projectRoot, casesPath) {
  const fromCases = loadCaseKeywordRegistry(casesPath);
  if (Object.keys(fromCases).length >= 2) {
    return { source: path.basename(casesPath), registry: fromCases };
  }
  return { source: "sy-hooks.policy.json", registry: loadPolicyKeywordRegistry(projectRoot) };
}

function scoreByKeywords(prompt, keywordRegistry) {
  const text = normalize(prompt);
  const rows = [];
  for (const [skill, keywords] of Object.entries(keywordRegistry)) {
    const list = Array.isArray(keywords) ? keywords : [];
    const matched = list.filter((kw) => kw && text.includes(kw));
    rows.push({
      skill,
      score: matched.length,
      matched,
    });
  }
  rows.sort((a, b) => b.score - a.score || a.skill.localeCompare(b.skill));
  return rows;
}

function commandExists(command, cwd) {
  const token = String(command || "").trim().split(/\s+/)[0];
  if (!token) return false;
  if (token.includes("/") || token.includes("\\") || path.isAbsolute(token)) {
    return fs.existsSync(path.isAbsolute(token) ? token : path.resolve(cwd, token));
  }
  if (process.platform === "win32") {
    const run = spawnSync("where", [token], { cwd, encoding: "utf8", shell: false });
    return run.status === 0;
  }
  const run = spawnSync("which", [token], { cwd, encoding: "utf8", shell: false });
  return run.status === 0;
}

function reportCommon({
  mode,
  skill,
  promptPath,
  projectRoot,
  logPath,
  triggered,
  observedSkills,
  extraLines,
}) {
  console.log("=== Skill Triggering Test ===");
  console.log(`mode: ${mode}`);
  console.log(`skill: ${skill}`);
  console.log(`prompt: ${path.relative(projectRoot, promptPath).replace(/\\/g, "/")}`);
  console.log(`log: ${path.relative(projectRoot, logPath).replace(/\\/g, "/")}`);
  console.log(`triggered: ${triggered ? "YES" : "NO"}`);
  console.log(`observedSkills: ${observedSkills.length > 0 ? observedSkills.join(", ") : "(none)"}`);
  for (const line of extraLines) {
    console.log(line);
  }
}

function runLocalMode({ projectRoot, skill, promptPath, prompt, outputDir, casesPath }) {
  const logPath = path.join(outputDir, "local.json");
  const skillPath = path.resolve(projectRoot, ".agents", "skills", skill, "SKILL.md");
  if (!fs.existsSync(skillPath)) {
    const result = { mode: "local", skill, error: `skill file not found: ${skillPath}` };
    fs.writeFileSync(logPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");
    reportCommon({
      mode: "local",
      skill,
      promptPath,
      projectRoot,
      logPath,
      triggered: false,
      observedSkills: [],
      extraLines: [`error: ${result.error}`],
    });
    return 1;
  }

  const skillDoc = fs.readFileSync(skillPath, "utf8");
  const frontmatter = parseFrontmatter(skillDoc);
  const fmName = String(frontmatter.name || "").trim();
  const fmDescription = String(frontmatter.description || "").trim();
  const useWhenOk = /^Use when\b/i.test(fmDescription);
  const nameOk = fmName === skill;

  const { source: keywordSource, registry } = resolveKeywordRegistry(projectRoot, casesPath);
  const targetKeywords = Array.isArray(registry[skill]) ? registry[skill] : [];
  const scored = scoreByKeywords(prompt, registry);
  const topScore = scored.length > 0 ? Number(scored[0].score || 0) : 0;
  const topSkills = scored.filter((row) => row.score === topScore && row.score > 0).map((row) => row.skill);
  const targetRow = scored.find((row) => row.skill === skill) || { score: 0, matched: [] };
  const triggered = topScore > 0 && topSkills.includes(skill);
  const observedSkills = scored.filter((row) => row.score > 0).map((row) => `${row.skill}(${row.score})`);

  const result = {
    mode: "local",
    skill,
    prompt: path.relative(projectRoot, promptPath).replace(/\\/g, "/"),
    keyword_source: keywordSource,
    checks: {
      frontmatter_name_match: nameOk,
      description_use_when: useWhenOk,
      target_keywords_present: targetKeywords.length > 0,
    },
    scored,
    triggered,
  };
  fs.writeFileSync(logPath, `${JSON.stringify(result, null, 2)}\n`, "utf8");

  reportCommon({
    mode: "local",
    skill,
    promptPath,
    projectRoot,
    logPath,
    triggered,
    observedSkills,
    extraLines: [
      `frontmatterNameMatch: ${nameOk ? "YES" : "NO"} (${fmName || "(missing)"})`,
      `descriptionUseWhen: ${useWhenOk ? "YES" : "NO"}`,
      `keywordSource: ${keywordSource}`,
      `targetKeywordHits: ${targetRow.score}`,
      `targetKeywordsConfigured: ${targetKeywords.length}`,
      `topSkills: ${topSkills.length > 0 ? topSkills.join(", ") : "(none)"}`,
    ],
  });

  if (!nameOk || !useWhenOk || targetKeywords.length === 0) {
    return 1;
  }
  return triggered ? 0 : 1;
}

function runRunnerMode({
  projectRoot,
  skill,
  promptPath,
  prompt,
  outputDir,
  runner,
  pluginDir,
  maxTurns,
  timeoutMs,
  casesPath,
}) {
  const logPath = path.join(outputDir, "stream.json");
  const detectPath = path.join(outputDir, "runner-detect.json");
  const commandArgs = [
    "-p",
    prompt,
    "--dangerously-skip-permissions",
    "--max-turns",
    Number.isFinite(maxTurns) ? String(maxTurns) : "3",
    "--output-format",
    "stream-json",
  ];
  if (pluginDir) {
    commandArgs.push("--plugin-dir", pluginDir);
  }

  const run = spawnSync(runner, commandArgs, {
    cwd: projectRoot,
    encoding: "utf8",
    timeout: Number.isFinite(timeoutMs) ? timeoutMs : 300000,
    shell: process.platform === "win32",
  });
  const timedOut = Boolean(run.error && String(run.error.code || "") === "ETIMEDOUT");

  const stdout = String(run.stdout || "");
  const stderr = String(run.stderr || "");
  const merged = `${stdout}\n${stderr}`.trim();
  fs.writeFileSync(logPath, `${merged}\n`, "utf8");

  const hasSkillTool = /"name":"Skill"|\"tool_name\":\"Skill\"/.test(merged);
  const expectedSkillRegex = new RegExp(`"skill":"([^"]*:)?${escapeRegex(skill)}"`, "i");
  const eventTriggered = hasSkillTool && expectedSkillRegex.test(merged);
  const observed = Array.from(merged.matchAll(/"skill":"([^"]+)"/g))
    .map((m) => String(m[1] || "").trim())
    .filter(Boolean);
  const observedUnique = unique(observed);

  // Fallback: some runners do not expose Skill tool events in stream output.
  // In that case, use keyword scoring across prompt + response text.
  const { source: keywordSource, registry } = resolveKeywordRegistry(projectRoot, casesPath);
  const combined = `${prompt}\n${merged}`;
  const scored = scoreByKeywords(combined, registry);
  const topScore = scored.length > 0 ? Number(scored[0].score || 0) : 0;
  const topSkills = scored.filter((row) => row.score === topScore && row.score > 0).map((row) => row.skill);
  const targetRow = scored.find((row) => row.skill === skill) || { score: 0, matched: [] };
  const semanticTriggered = topScore > 0 && topSkills.includes(skill);
  const triggered = eventTriggered || semanticTriggered;
  const detectionSource = eventTriggered ? "event" : semanticTriggered ? "semantic-fallback" : "none";

  const detectSummary = {
    mode: "runner",
    skill,
    prompt: path.relative(projectRoot, promptPath).replace(/\\/g, "/"),
    runner,
    runner_exit_code: run.status,
    detection_source: detectionSource,
    event_triggered: eventTriggered,
    semantic_triggered: semanticTriggered,
    keyword_source: keywordSource,
    target_score: targetRow.score,
    target_matched: targetRow.matched,
    top_skills: topSkills,
    observed_skill_events: observedUnique,
  };
  fs.writeFileSync(detectPath, `${JSON.stringify(detectSummary, null, 2)}\n`, "utf8");

  reportCommon({
    mode: "runner",
    skill,
    promptPath,
    projectRoot,
    logPath,
    triggered,
    observedSkills: observedUnique,
    extraLines: [
      `runner: ${runner}`,
      pluginDir ? `pluginDir: ${pluginDir}` : "",
      `exitCode: ${run.status}`,
      timedOut ? "timeoutDetected: YES" : "timeoutDetected: NO",
      run.error ? `runnerError: ${run.error.message}` : "",
      `detectionSource: ${detectionSource}`,
      `semanticTargetHits: ${targetRow.score}`,
      `keywordSource: ${keywordSource}`,
      `detectLog: ${path.relative(projectRoot, detectPath).replace(/\\/g, "/")}`,
    ].filter(Boolean),
  });

  if (run.error && run.error.code === "ENOENT") {
    return 2;
  }
  if (timedOut && !triggered) {
    return 1;
  }
  return triggered ? 0 : 1;
}

function main() {
  const args = parseArgs(process.argv);
  const skill = String(args.skill || "").trim();
  const promptFile = String(args.prompt || "").trim();
  const runner = String(args.runner || process.env.SY_SKILL_TEST_RUNNER || "claude").trim();
  const pluginDir = String(args["plugin-dir"] || process.env.SY_SKILL_TEST_PLUGIN_DIR || "").trim();
  const modeInput = normalize(args.mode || process.env.SY_SKILL_TEST_MODE || "auto");
  const mode = ["auto", "runner", "local"].includes(modeInput) ? modeInput : "auto";
  const maxTurns = Number(args["max-turns"] || 3);
  const timeoutMs = Number(args["timeout-ms"] || 300000);
  const casesFile = String(args.cases || "tests/skill-triggering/cases.json").trim();

  if (!skill || !promptFile) {
    console.error(
      "Usage: node tests/skill-triggering/run-test.cjs --skill <skill-name> --prompt <prompt-file> [--mode auto|runner|local]",
    );
    process.exit(2);
  }

  const projectRoot = resolveProjectRoot();
  const casesPath = path.isAbsolute(casesFile) ? casesFile : path.resolve(projectRoot, casesFile);
  if (!fs.existsSync(casesPath)) {
    console.error(`[skill-triggering] cases file not found: ${casesPath}`);
    process.exit(2);
  }
  const promptPath = path.isAbsolute(promptFile) ? promptFile : path.resolve(projectRoot, promptFile);
  if (!fs.existsSync(promptPath)) {
    console.error(`[skill-triggering] prompt file not found: ${promptPath}`);
    process.exit(2);
  }

  const prompt = fs.readFileSync(promptPath, "utf8");
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  const outputDir = path.resolve(projectRoot, "tests", "skill-triggering", "output", timestamp, skill);
  ensureDir(outputDir);

  let effectiveMode = mode;
  if (mode === "auto") {
    effectiveMode = commandExists(runner, projectRoot) ? "runner" : "local";
  }

  if (effectiveMode === "local") {
    process.exit(
      runLocalMode({
        projectRoot,
        skill,
        promptPath,
        prompt,
        outputDir,
        casesPath,
      }),
    );
  }

  process.exit(
    runRunnerMode({
      projectRoot,
      skill,
      promptPath,
      prompt,
      outputDir,
      runner,
      pluginDir,
      maxTurns,
      timeoutMs,
      casesPath,
    }),
  );
}

main();
