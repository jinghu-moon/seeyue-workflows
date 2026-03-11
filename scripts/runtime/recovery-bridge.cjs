#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { appendEvent } = require("./journal.cjs");
const { buildResumeFrontier, restoreCheckpoint } = require("./checkpoints.cjs");
const { readSession, readSprintStatus, writeSession, writeSprintStatus } = require("./store.cjs");

function nowIso() {
  return new Date().toISOString();
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function resolveCheckpointId(rootDir, checkpointId) {
  if (checkpointId) {
    return String(checkpointId).trim();
  }
  const session = readSession(rootDir);
  const lastCheckpointId = String(session?.recovery?.last_checkpoint_id || "").trim();
  if (!lastCheckpointId) {
    throw new Error("RECOVERY_BRIDGE_MISSING_CHECKPOINT_ID");
  }
  return lastCheckpointId;
}

function markRestorePending(rootDir, reason, actor = "adapter") {
  const session = readSession(rootDir);
  if (!session) {
    throw new Error("session.yaml is required for recovery bridge");
  }
  const updated = clone(session);
  updated.recovery = {
    ...updated.recovery,
    restore_pending: true,
    restore_reason: reason,
  };
  updated.timestamps = {
    ...updated.timestamps,
    updated_at: nowIso(),
  };
  writeSession(rootDir, updated);
  appendEvent(rootDir, {
    runId: updated.run_id,
    event: "validation_failed",
    phase: updated.phase?.current || "none",
    nodeId: updated.node?.active_id || "none",
    actor,
    payload: {
      reason,
      recovery_bridge: true,
    },
  });
  return updated;
}

function readJsonFile(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function loadGeminiCheckpointEnvelope(checkpointFilePath) {
  const absolutePath = path.resolve(checkpointFilePath);
  if (!fs.existsSync(absolutePath)) {
    throw new Error(`GEMINI_RESTORE_CHECKPOINT_NOT_FOUND path=${absolutePath}`);
  }
  const envelope = readJsonFile(absolutePath);
  if (!isObject(envelope)) {
    throw new Error(`GEMINI_RESTORE_INVALID_CHECKPOINT path=${absolutePath} reason=invalid_json_object`);
  }
  if (!Array.isArray(envelope.history)) {
    throw new Error(`GEMINI_RESTORE_INVALID_CHECKPOINT path=${absolutePath} reason=missing_history`);
  }
  if (!isObject(envelope.toolCall)) {
    throw new Error(`GEMINI_RESTORE_INVALID_CHECKPOINT path=${absolutePath} reason=missing_tool_call_metadata`);
  }
  const toolName = String(envelope.toolCall.toolName || "").trim();
  if (!toolName) {
    throw new Error(`GEMINI_RESTORE_INVALID_CHECKPOINT path=${absolutePath} reason=missing_tool_name`);
  }
  return {
    file_path: absolutePath.replace(/\\/g, "/"),
    history: envelope.history,
    tool_call: {
      tool_name: toolName,
      tool_args: isObject(envelope.toolCall.toolArgs) ? envelope.toolCall.toolArgs : {},
    },
    raw: envelope,
  };
}

function buildRecoveryRecommendedNext(checkpoint, geminiEnvelope) {
  return {
    type: "resume_node",
    target: checkpoint.node_id,
    params: {
      checkpoint_id: checkpoint.checkpoint_id,
      resume_mode: "gemini_restore",
      tool_name: geminiEnvelope.tool_call.tool_name,
    },
    reason: "resume from restored checkpoint and replay the saved Gemini tool call if still needed",
    blocking_on: [],
    priority: "now",
  };
}

function upsertSprintStatus(rootDir, checkpoint, geminiEnvelope) {
  const sprintStatus = readSprintStatus(rootDir);
  if (!sprintStatus) {
    return null;
  }
  const updated = clone(sprintStatus);
  updated.active_phase = checkpoint.phase;
  updated.recommended_next = [buildRecoveryRecommendedNext(checkpoint, geminiEnvelope)];
  if (Array.isArray(updated.node_summary)) {
    updated.node_summary = updated.node_summary.map((entry) => {
      if (!entry || entry.id !== checkpoint.node_id) {
        return entry;
      }
      return {
        ...entry,
        status: "in_progress",
      };
    });
  }
  writeSprintStatus(rootDir, updated);
  return updated;
}

function bridgeGeminiRestore(rootDir, options = {}) {
  const actor = options.actor || "adapter";
  const resolvedCheckpointId = resolveCheckpointId(rootDir, options.checkpointId);

  let geminiEnvelope;
  try {
    geminiEnvelope = loadGeminiCheckpointEnvelope(options.geminiCheckpointPath);
  } catch (error) {
    if (/missing_tool_call_metadata/.test(String(error.message || ""))) {
      markRestorePending(rootDir, "missing_tool_call_metadata", actor);
    }
    throw error;
  }

  const restoredCheckpoint = restoreCheckpoint(rootDir, resolvedCheckpointId, { actor });
  upsertSprintStatus(rootDir, restoredCheckpoint, geminiEnvelope);
  const session = readSession(rootDir);
  appendEvent(rootDir, {
    runId: session?.run_id,
    event: "session_resumed",
    phase: restoredCheckpoint.phase,
    nodeId: restoredCheckpoint.node_id,
    actor,
    payload: {
      source: "gemini_restore",
      checkpoint_id: restoredCheckpoint.checkpoint_id,
      checkpoint_file: geminiEnvelope.file_path,
      history_items: geminiEnvelope.history.length,
      tool_name: geminiEnvelope.tool_call.tool_name,
    },
  });

  const resumeFrontier = buildResumeFrontier(rootDir);
  return {
    checkpoint_id: restoredCheckpoint.checkpoint_id,
    restored_checkpoint: restoredCheckpoint,
    tool_replay: clone(geminiEnvelope.tool_call),
    resume_frontier: resumeFrontier,
    checkpoint_file: geminiEnvelope.file_path,
    history_items: geminiEnvelope.history.length,
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    checkpointId: null,
    geminiCheckpointPath: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--checkpoint-id":
        index += 1;
        parsed.checkpointId = argv[index];
        break;
      case "--gemini-checkpoint":
        index += 1;
        parsed.geminiCheckpointPath = argv[index];
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }
  if (!parsed.geminiCheckpointPath) {
    throw new Error("Missing required argument: --gemini-checkpoint");
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
    const result = bridgeGeminiRestore(parsed.rootDir, parsed);
    console.log(JSON.stringify(result, null, 2));
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  bridgeGeminiRestore,
  buildRecoveryRecommendedNext,
  loadGeminiCheckpointEnvelope,
  markRestorePending,
  resolveCheckpointId,
};

if (require.main === module) {
  main();
}
