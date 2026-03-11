#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

function normalizePath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function parseJsonSafe(value) {
  try {
    return JSON.parse(String(value || "{}"));
  } catch {
    return {};
  }
}

function readStdin() {
  return fs.readFileSync(0, "utf8");
}

function parseArgs(argv) {
  const parsed = {
    mode: "",
    delegate: "",
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--mode":
        index += 1;
        parsed.mode = String(argv[index] || "").trim();
        break;
      case "--delegate":
        index += 1;
        parsed.delegate = String(argv[index] || "").trim();
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }

  if (!parsed.mode) {
    throw new Error("Missing required argument: --mode");
  }
  if (!parsed.delegate) {
    throw new Error("Missing required argument: --delegate");
  }

  return parsed;
}

function normalizePayload(mode, payload) {
  const normalized = payload && typeof payload === "object" ? { ...payload } : {};
  if (normalized.tool_name && !normalized.tool) {
    normalized.tool = normalized.tool_name;
  }
  if (mode === "after-agent" && normalized.prompt_response && !normalized.last_assistant_message) {
    normalized.last_assistant_message = normalized.prompt_response;
  }
  return normalized;
}

function extractContext(parsed) {
  if (!parsed || typeof parsed !== "object") {
    return "";
  }
  return String(
    parsed.hookSpecificOutput?.additionalContext
      || parsed.additional_context
      || parsed.additionalContext
      || parsed.context
      || "",
  ).trim();
}

function extractHookSpecificOutput(parsed) {
  if (!parsed || typeof parsed !== "object") {
    return null;
  }
  const output = parsed.hookSpecificOutput;
  if (!output || typeof output !== "object" || Array.isArray(output)) {
    return null;
  }
  return output;
}

function toGeminiSuccess(mode, parsed) {
  const result = {};
  const context = extractContext(parsed);
  const hookSpecificOutput = extractHookSpecificOutput(parsed);
  const toolConfig = parsed?.toolConfig && typeof parsed.toolConfig === "object" ? parsed.toolConfig : null;
  const decision = parsed?.decision;
  const continueFlag = parsed?.continue;
  const systemMessage = String(parsed?.systemMessage || "").trim();

  if (mode === "session-start" || mode === "before-agent" || mode === "after-tool") {
    const merged = hookSpecificOutput ? { ...hookSpecificOutput } : {};
    if (context) {
      merged.additionalContext = context;
    }
    if (Object.keys(merged).length > 0) {
      result.hookSpecificOutput = merged;
    }
  } else if (mode === "after-agent") {
    if (context) {
      result.systemMessage = context;
    }
  } else if (mode === "before-tool-selection") {
    const merged = hookSpecificOutput ? { ...hookSpecificOutput } : {};
    if (toolConfig && !merged.toolConfig) {
      merged.toolConfig = toolConfig;
    }
    if (Object.keys(merged).length > 0) {
      result.hookSpecificOutput = merged;
    }
  } else if (mode === "after-model") {
    if (hookSpecificOutput && Object.keys(hookSpecificOutput).length > 0) {
      result.hookSpecificOutput = hookSpecificOutput;
    }
    if (decision !== undefined) {
      result.decision = decision;
    }
    if (continueFlag !== undefined) {
      result.continue = continueFlag;
    }
  }

  if (systemMessage) {
    result.systemMessage = systemMessage;
  }

  return result;
}

function toGeminiBlock(mode, message) {
  const reason = String(message || "Blocked by hook bridge.").trim() || "Blocked by hook bridge.";
  switch (mode) {
    case "before-tool":
    case "after-tool":
    case "before-agent":
    case "after-agent":
    case "before-tool-selection":
    case "after-model":
      return { decision: "deny", reason };
    case "session-start":
    default:
      return { systemMessage: reason };
  }
}

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    process.stderr.write(`ARG_PARSE_FAIL ${error.message}\n`);
    process.exit(1);
  }

  const raw = readStdin();
  const payload = normalizePayload(parsed.mode, parseJsonSafe(raw));
  const delegatePath = path.resolve(process.cwd(), parsed.delegate);
  if (!fs.existsSync(delegatePath)) {
    process.stderr.write(`DELEGATE_NOT_FOUND ${normalizePath(parsed.delegate)}\n`);
    process.exit(1);
  }

  const child = spawnSync(process.execPath, [delegatePath], {
    encoding: "utf8",
    input: `${JSON.stringify(payload)}\n`,
  });

  if (child.error) {
    process.stderr.write(`HOOK_BRIDGE_FAIL ${child.error.message}\n`);
    process.exit(1);
  }

  const stdoutText = String(child.stdout || "");
  const stderrText = String(child.stderr || "");
  if (stderrText) {
    process.stderr.write(stderrText);
  }

  if (child.status === 0) {
    const mapped = toGeminiSuccess(parsed.mode, parseJsonSafe(stdoutText));
    process.stdout.write(JSON.stringify(mapped));
    return;
  }

  if (child.status === 2) {
    const blockMessage = stderrText.trim() || stdoutText.trim() || `Blocked by ${path.basename(delegatePath)}`;
    process.stdout.write(JSON.stringify(toGeminiBlock(parsed.mode, blockMessage)));
    return;
  }

  process.stderr.write(`HOOK_BRIDGE_DELEGATE_FAIL status=${child.status || 1} delegate=${normalizePath(parsed.delegate)}\n`);
  process.exit(child.status || 1);
}

main();
