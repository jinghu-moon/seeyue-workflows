#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const { makeTempRoot } = require("../runtime/runtime-fixture-lib.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");
const fixturesDir = path.join(__dirname, "fixtures");
const pretoolWriteHook = path.join(projectRoot, "scripts", "hooks", "sy-pretool-write.cjs");
const geminiBridge = path.join(projectRoot, "scripts", "hooks", "gemini-hook-bridge.cjs");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function parseJsonSafe(text) {
  try {
    const parsed = JSON.parse(String(text || "{}"));
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function ensureKeys(obj, keys, label) {
  assert(obj && typeof obj === "object", `${label} must be an object`);
  for (const key of keys) {
    assert(key in obj, `${label} missing key: ${key}`);
  }
}

function runNode(scriptPath, args, payload, options = {}) {
  const result = spawnSync(process.execPath, [scriptPath, ...(args || [])], {
    cwd: options.cwd || process.cwd(),
    input: JSON.stringify(payload),
    encoding: "utf8",
    env: { ...process.env, ...(options.env || {}) },
  });
  return {
    code: Number.isInteger(result.status) ? result.status : 1,
    stdout: String(result.stdout || "").trim(),
    stderr: String(result.stderr || "").trim(),
  };
}

function writeDelegateScript(rootDir, name, output, exitCode, stderrText) {
  const scriptPath = path.join(rootDir, name);
  const payload = JSON.stringify(output || {});
  const scriptLines = [
    "#!/usr/bin/env node",
    "\"use strict\";",
    `process.stdout.write(${JSON.stringify(payload)});`,
    stderrText ? `process.stderr.write(${JSON.stringify(String(stderrText))});` : "",
    `process.exit(${Number.isInteger(exitCode) ? exitCode : 0});`,
  ].filter(Boolean);
  fs.writeFileSync(scriptPath, scriptLines.join("\n"), "utf8");
  return scriptPath;
}

function loadFixtures() {
  const claude = readJson(path.join(fixturesDir, "claude-pretooluse-minimal.json"));
  const gemini = readJson(path.join(fixturesDir, "gemini-beforetool-minimal.json"));
  const codex = readJson(path.join(fixturesDir, "codex-after-tool-use-minimal.json"));
  return { claude, gemini, codex };
}

function assertFixtureShapes(fixtures) {
  ensureKeys(fixtures.claude, [
    "hook_event_name",
    "session_id",
    "cwd",
    "transcript_path",
    "tool_name",
    "tool_input",
  ], "claude fixture");
  ensureKeys(fixtures.claude.tool_input, ["file_path", "content"], "claude tool_input");

  ensureKeys(fixtures.gemini, [
    "hook_event_name",
    "session_id",
    "cwd",
    "transcript_path",
    "timestamp",
    "tool_name",
    "tool_input",
  ], "gemini fixture");
  ensureKeys(fixtures.gemini.tool_input, ["path", "content"], "gemini tool_input");

  ensureKeys(fixtures.codex, ["session_id", "cwd", "triggered_at", "hook_event"], "codex fixture");
  ensureKeys(fixtures.codex.hook_event, [
    "event_type",
    "turn_id",
    "call_id",
    "tool_name",
    "tool_kind",
    "tool_input",
    "executed",
    "success",
    "duration_ms",
    "mutating",
    "sandbox",
    "sandbox_policy",
    "output_preview",
  ], "codex hook_event");
  ensureKeys(fixtures.codex.hook_event.tool_input, ["input_type", "params"], "codex tool_input");
}

function runClaudeMinimalAllow(fixtures) {
  const rootDir = makeTempRoot("sy-hooks-template-claude-");
  const payload = cloneJson(fixtures.claude);
  payload.cwd = rootDir;

  const result = runNode(pretoolWriteHook, [], payload, {
    cwd: rootDir,
    env: { SY_BYPASS_PRETOOL_WRITE: "1" },
  });
  const output = parseJsonSafe(result.stdout);

  assert(result.code === 0, `expected claude minimal allow exit 0 but got ${result.code}`);
  assert(output.verdict === "allow", `expected verdict=allow but got ${JSON.stringify(output)}`);
}

function runGeminiMinimalAllow(fixtures) {
  const rootDir = makeTempRoot("sy-hooks-template-gemini-");
  const payload = cloneJson(fixtures.gemini);
  payload.cwd = rootDir;

  const result = runNode(
    geminiBridge,
    ["--mode", "before-tool", "--delegate", pretoolWriteHook],
    payload,
    {
      cwd: rootDir,
      env: { SY_BYPASS_PRETOOL_WRITE: "1" },
    },
  );
  const output = parseJsonSafe(result.stdout);

  assert(result.code === 0, `expected gemini minimal allow exit 0 but got ${result.code}`);
  assert(!("decision" in output), `expected no decision for allow but got ${JSON.stringify(output)}`);
}

function runGeminiMinimalBlock(fixtures) {
  const rootDir = makeTempRoot("sy-hooks-template-gemini-block-");
  const payload = cloneJson(fixtures.gemini);
  payload.cwd = rootDir;
  payload.tool_input.path = ".env";

  const result = runNode(
    geminiBridge,
    ["--mode", "before-tool", "--delegate", pretoolWriteHook],
    payload,
    { cwd: rootDir },
  );
  const output = parseJsonSafe(result.stdout);

  assert(result.code === 0, `expected gemini minimal block exit 0 but got ${result.code}`);
  assert(output.decision === "deny", `expected decision=deny but got ${JSON.stringify(output)}`);
  assert(String(output.reason || "").length > 0, "expected deny reason to be non-empty");
}

function runGeminiAfterToolContextMapping() {
  const rootDir = makeTempRoot("sy-hooks-template-gemini-context-");
  const delegate = writeDelegateScript(
    rootDir,
    "delegate-context.cjs",
    { additionalContext: "ctx", systemMessage: "msg" },
    0,
  );
  const payload = {
    hook_event_name: "AfterTool",
    session_id: "s-ctx",
    cwd: rootDir,
    transcript_path: "/tmp/gemini.json",
    timestamp: "2025-01-01T00:00:00Z",
    tool_name: "write_file",
    tool_input: { path: "docs/notes.md", content: "hello" },
    tool_response: { returnDisplay: "ok" },
  };

  const result = runNode(
    geminiBridge,
    ["--mode", "after-tool", "--delegate", delegate],
    payload,
    { cwd: rootDir },
  );
  const output = parseJsonSafe(result.stdout);

  assert(result.code === 0, `expected gemini context mapping exit 0 but got ${result.code}`);
  assert(output.systemMessage === "msg", `expected systemMessage=msg but got ${JSON.stringify(output)}`);
  assert(
    output.hookSpecificOutput && output.hookSpecificOutput.additionalContext === "ctx",
    `expected additionalContext=ctx but got ${JSON.stringify(output)}`,
  );
}

function runGeminiToolSelectionMapping() {
  const rootDir = makeTempRoot("sy-hooks-template-gemini-toolcfg-");
  const delegate = writeDelegateScript(
    rootDir,
    "delegate-toolcfg.cjs",
    { hookSpecificOutput: { toolConfig: { mode: "ANY", allowedFunctionNames: ["read_file"] } } },
    0,
  );
  const payload = {
    hook_event_name: "BeforeToolSelection",
    session_id: "s-toolcfg",
    cwd: rootDir,
    transcript_path: "/tmp/gemini.json",
    timestamp: "2025-01-01T00:00:00Z",
  };

  const result = runNode(
    geminiBridge,
    ["--mode", "before-tool-selection", "--delegate", delegate],
    payload,
    { cwd: rootDir },
  );
  const output = parseJsonSafe(result.stdout);

  assert(result.code === 0, `expected toolConfig mapping exit 0 but got ${result.code}`);
  assert(
    output.hookSpecificOutput && output.hookSpecificOutput.toolConfig,
    `expected toolConfig in output but got ${JSON.stringify(output)}`,
  );
  assert(
    output.hookSpecificOutput.toolConfig.mode === "ANY",
    `expected toolConfig.mode=ANY but got ${JSON.stringify(output)}`,
  );
}

function runGeminiAfterModelMapping() {
  const rootDir = makeTempRoot("sy-hooks-template-gemini-aftermodel-");
  const delegate = writeDelegateScript(
    rootDir,
    "delegate-aftermodel.cjs",
    {
      hookSpecificOutput: { llm_response: { candidates: [] } },
      decision: "deny",
      continue: false,
    },
    0,
  );
  const payload = {
    hook_event_name: "AfterModel",
    session_id: "s-aftermodel",
    cwd: rootDir,
    transcript_path: "/tmp/gemini.json",
    timestamp: "2025-01-01T00:00:00Z",
    llm_request: { model: "test", messages: [] },
    llm_response: { candidates: [] },
  };

  const result = runNode(
    geminiBridge,
    ["--mode", "after-model", "--delegate", delegate],
    payload,
    { cwd: rootDir },
  );
  const output = parseJsonSafe(result.stdout);

  assert(result.code === 0, `expected after-model mapping exit 0 but got ${result.code}`);
  assert(output.decision === "deny", `expected decision=deny but got ${JSON.stringify(output)}`);
  assert(output.continue === false, `expected continue=false but got ${JSON.stringify(output)}`);
  assert(
    output.hookSpecificOutput && output.hookSpecificOutput.llm_response,
    `expected llm_response mapping but got ${JSON.stringify(output)}`,
  );
}

const CASES = {
  "fixture-shapes": assertFixtureShapes,
  "claude-minimal-allow": runClaudeMinimalAllow,
  "gemini-minimal-allow": runGeminiMinimalAllow,
  "gemini-minimal-block": runGeminiMinimalBlock,
  "gemini-context-mapping": runGeminiAfterToolContextMapping,
  "gemini-toolcfg-mapping": runGeminiToolSelectionMapping,
  "gemini-aftermodel-mapping": runGeminiAfterModelMapping,
};

function main() {
  const fixtures = loadFixtures();
  let failed = false;

  for (const [caseName, runner] of Object.entries(CASES)) {
    try {
      if (runner.length === 1) {
        runner(fixtures);
      } else {
        runner();
      }
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
    }
  }

  if (failed) {
    process.exit(1);
  }
  console.log("HOOK_TEMPLATE_FIXTURES_PASS");
}

main();
