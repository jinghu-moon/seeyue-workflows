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

function main() {
  const args = parseArgs(process.argv);
  const maxTurns = String(args["max-turns"] || 3);
  const timeoutMs = String(args["timeout-ms"] || 300000);
  const runner = String(args.runner || process.env.SY_SKILL_TEST_RUNNER || "claude");
  const pluginDir = String(args["plugin-dir"] || process.env.SY_SKILL_TEST_PLUGIN_DIR || "");
  const mode = String(args.mode || process.env.SY_SKILL_TEST_MODE || "auto");
  const casesFile = String(args.cases || "tests/skill-triggering/cases.json");

  const projectRoot = path.resolve(__dirname, "..", "..");
  const casesPath = path.isAbsolute(casesFile) ? casesFile : path.resolve(projectRoot, casesFile);
  const runTestPath = path.resolve(__dirname, "run-test.cjs");
  if (!fs.existsSync(casesPath)) {
    console.error(`[skill-triggering] missing cases: ${casesPath}`);
    process.exit(2);
  }

  const raw = fs.readFileSync(casesPath, "utf8");
  const cases = JSON.parse(raw);
  if (!Array.isArray(cases) || cases.length === 0) {
    console.error("[skill-triggering] cases.json is empty");
    process.exit(2);
  }

  let pass = 0;
  let fail = 0;

  for (const row of cases) {
    const entry = row && typeof row === "object" ? row : {};
    const skill = String(entry.skill || "").trim();
    const prompt = String(entry.prompt || "").trim();
    if (!skill || !prompt) {
      console.error("[skill-triggering] invalid case entry");
      fail += 1;
      continue;
    }

    const run = spawnSync(process.execPath, [
      runTestPath,
      "--skill",
      skill,
      "--prompt",
      prompt,
      "--max-turns",
      maxTurns,
      "--timeout-ms",
      timeoutMs,
      "--runner",
      runner,
      "--mode",
      mode,
      "--cases",
      casesPath,
      ...(pluginDir ? ["--plugin-dir", pluginDir] : []),
    ], {
      cwd: projectRoot,
      encoding: "utf8",
    });

    process.stdout.write(String(run.stdout || ""));
    process.stderr.write(String(run.stderr || ""));

    if (run.status === 0) {
      pass += 1;
    } else {
      fail += 1;
    }
  }

  console.log("=== Summary ===");
  console.log(`mode: ${mode}`);
  console.log(`cases: ${path.relative(projectRoot, casesPath).replace(/\\/g, "/")}`);
  console.log(`pass: ${pass}`);
  console.log(`fail: ${fail}`);
  console.log(`total: ${pass + fail}`);

  process.exit(fail === 0 ? 0 : 1);
}

main();
