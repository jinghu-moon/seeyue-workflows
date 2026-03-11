"use strict";

const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");
const { dumpYamlFile, loadYamlFile } = require("./yaml-loader.cjs");
const { refreshLedger } = require("./ledger.cjs");
const {
  listCapsules,
  readCheckpoint,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeCapsule,
  writeCheckpoint,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("./store.cjs");
const {
  appendEvent,
  findLatestEvent,
  getJournalOffset,
  readEvents,
} = require("./journal.cjs");

const CHECKPOINT_CLASSES = new Set(["node", "review", "pre_destructive"]);
const TERMINAL_NODE_EVENTS = new Set(["node_completed", "node_failed", "node_timed_out", "node_bypassed"]);
const INLINE_TARGET_SNAPSHOT_LIMIT_BYTES = 256 * 1024;

function nowIso() {
  return new Date().toISOString();
}

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function checkpointDir(rootDir) {
  return path.join(path.resolve(rootDir), ".ai", "workflow", "checkpoints");
}

function ensureCheckpointDir(rootDir) {
  fs.mkdirSync(checkpointDir(rootDir), { recursive: true });
}

function buildCheckpointId(checkpointClass) {
  return `${checkpointClass}-${Date.now()}-${crypto.randomBytes(4).toString("hex")}`;
}

function relativeCheckpointRef(checkpointId, suffix) {
  return path.posix.join(".ai", "workflow", "checkpoints", `${checkpointId}.${suffix}`);
}

function absoluteFromRelative(rootDir, relativePath) {
  const segments = relativePath.split("/");
  return path.join(path.resolve(rootDir), ...segments);
}

function computeIntegrityHash(parts) {
  const hash = crypto.createHash("sha256");
  for (const part of parts) {
    hash.update(typeof part === "string" ? part : JSON.stringify(part));
  }
  return hash.digest("hex");
}

function persistYamlSnapshot(rootDir, relativeRef, value) {
  const targetPath = absoluteFromRelative(rootDir, relativeRef);
  dumpYamlFile(targetPath, value);
  return relativeRef;
}

function persistJsonSnapshot(rootDir, relativeRef, value) {
  const targetPath = absoluteFromRelative(rootDir, relativeRef);
  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.writeFileSync(targetPath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
  return relativeRef;
}

function persistTextSnapshot(rootDir, relativeRef, value) {
  const targetPath = absoluteFromRelative(rootDir, relativeRef);
  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.writeFileSync(targetPath, value, "utf8");
  return relativeRef;
}

function computeHash(content) {
  return crypto.createHash("sha256").update(content).digest("hex");
}

function readGitHeadRef(rootDir) {
  const resolvedRoot = path.resolve(rootDir);
  const gitResult = spawnSync("git", ["-C", resolvedRoot, "rev-parse", "HEAD"], {
    encoding: "utf8",
    windowsHide: true,
  });
  if (gitResult.status === 0) {
    const value = String(gitResult.stdout || "").trim();
    if (/^[0-9a-f]{40}$/i.test(value)) {
      return value;
    }
  }

  const headPath = path.join(resolvedRoot, ".git", "HEAD");
  if (!fs.existsSync(headPath)) {
    return null;
  }
  const headValue = String(fs.readFileSync(headPath, "utf8") || "").trim();
  if (/^[0-9a-f]{40}$/i.test(headValue)) {
    return headValue;
  }
  const refMatch = headValue.match(/^ref:\s+(.+)$/i);
  if (!refMatch) {
    return null;
  }
  const refPath = path.join(resolvedRoot, ".git", ...String(refMatch[1]).trim().split("/"));
  if (!fs.existsSync(refPath)) {
    return null;
  }
  const refValue = String(fs.readFileSync(refPath, "utf8") || "").trim();
  return /^[0-9a-f]{40}$/i.test(refValue) ? refValue : null;
}

function captureLatestCapsuleSnapshot(rootDir, checkpointId) {
  const latestCapsule = listCapsules(rootDir)[0] || null;
  if (!latestCapsule) {
    return {
      capsule_snapshot_ref: null,
      capsule_id: null,
    };
  }
  const snapshotRef = relativeCheckpointRef(checkpointId, "capsule.json");
  persistJsonSnapshot(rootDir, snapshotRef, latestCapsule);
  return {
    capsule_snapshot_ref: snapshotRef,
    capsule_id: latestCapsule.capsule_id || null,
  };
}

function restoreCapsuleSnapshot(rootDir, checkpoint) {
  const snapshotRef = String(checkpoint?.capsule_snapshot_ref || "").trim();
  if (!snapshotRef) {
    return null;
  }
  const snapshotPath = absoluteFromRelative(rootDir, snapshotRef);
  if (!fs.existsSync(snapshotPath)) {
    return {
      restored: false,
      capsule_id: String(checkpoint?.capsule_id || "").trim() || null,
      restore_reason: "capsule_snapshot_missing",
    };
  }
  const capsule = JSON.parse(fs.readFileSync(snapshotPath, "utf8"));
  if (!capsule || typeof capsule !== "object" || typeof capsule.capsule_id !== "string" || capsule.capsule_id.length === 0) {
    return {
      restored: false,
      capsule_id: String(checkpoint?.capsule_id || "").trim() || null,
      restore_reason: "capsule_snapshot_invalid",
    };
  }
  writeCapsule(rootDir, capsule);
  return {
    restored: true,
    capsule_id: capsule.capsule_id,
    restore_reason: null,
  };
}

function captureTargetSnapshot(rootDir, checkpointId, options = {}) {
  const targetRef = String(options.targetRef || "").trim();
  const operationKind = String(options.operationKind || "").trim();
  if (!targetRef || !["write", "edit"].includes(operationKind)) {
    return {
      target_snapshot_ref: null,
      target_snapshot_content_ref: null,
    };
  }

  const absoluteTargetPath = path.isAbsolute(targetRef) ? targetRef : path.resolve(rootDir, targetRef);
  const existsBefore = fs.existsSync(absoluteTargetPath);
  const metadata = {
    target_ref: targetRef,
    absolute_path: absoluteTargetPath.replace(/\\/g, "/"),
    exists_before: existsBefore,
    kind: existsBefore && fs.statSync(absoluteTargetPath).isFile() ? "file" : existsBefore ? "other" : "missing",
    size_bytes: existsBefore && fs.statSync(absoluteTargetPath).isFile() ? fs.statSync(absoluteTargetPath).size : 0,
    sha256: null,
    content_ref: null,
    content_encoding: null,
    capture_mode: existsBefore ? "metadata_only" : "delete_only",
    capture_limit_bytes: INLINE_TARGET_SNAPSHOT_LIMIT_BYTES,
    capture_reason: existsBefore ? "unsupported_target_kind" : null,
  };

  if (existsBefore && metadata.kind === "file") {
    const buffer = fs.readFileSync(absoluteTargetPath);
    metadata.sha256 = computeHash(buffer);
    if (buffer.length <= INLINE_TARGET_SNAPSHOT_LIMIT_BYTES) {
      const contentRef = relativeCheckpointRef(checkpointId, "target.before.txt");
      persistTextSnapshot(rootDir, contentRef, buffer.toString("utf8"));
      metadata.content_ref = contentRef;
      metadata.content_encoding = "utf8";
      metadata.capture_mode = "inline";
      metadata.capture_reason = null;
    } else {
      metadata.capture_mode = "metadata_only";
      metadata.capture_reason = "size_limit_exceeded";
    }
  }

  const snapshotRef = relativeCheckpointRef(checkpointId, "target.json");
  persistJsonSnapshot(rootDir, snapshotRef, metadata);
  return {
    target_snapshot_ref: snapshotRef,
    target_snapshot_content_ref: metadata.content_ref,
  };
}

function restoreTargetSnapshot(rootDir, checkpoint) {
  const snapshotRef = String(checkpoint?.target_snapshot_ref || "").trim();
  if (!snapshotRef) {
    return null;
  }
  const metadataPath = absoluteFromRelative(rootDir, snapshotRef);
  if (!fs.existsSync(metadataPath)) {
    return {
      target_ref: String(checkpoint?.target_ref || "").trim() || null,
      restored: false,
      requires_manual_restore: true,
      restore_reason: "target_snapshot_metadata_missing",
      metadata: null,
    };
  }
  const metadata = JSON.parse(fs.readFileSync(metadataPath, "utf8"));
  const targetRef = String(metadata?.target_ref || "").trim();
  if (!targetRef) {
    return {
      target_ref: null,
      restored: false,
      requires_manual_restore: true,
      restore_reason: "target_snapshot_metadata_missing",
      metadata,
    };
  }

  const absoluteTargetPath = path.isAbsolute(targetRef) ? targetRef : path.resolve(rootDir, targetRef);
  if (metadata.exists_before === false) {
    if (fs.existsSync(absoluteTargetPath)) {
      fs.rmSync(absoluteTargetPath, { force: true });
    }
    return {
      target_ref: targetRef,
      restored: true,
      requires_manual_restore: false,
      restore_reason: null,
      restore_action: "delete_new_target",
      metadata,
    };
  }
  if (metadata.kind && metadata.kind !== "file") {
    return {
      target_ref: targetRef,
      restored: false,
      requires_manual_restore: true,
      restore_reason: "target_snapshot_unsupported_kind",
      metadata,
    };
  }
  if (metadata.content_ref) {
    const contentPath = absoluteFromRelative(rootDir, metadata.content_ref);
    if (!fs.existsSync(contentPath)) {
      return {
        target_ref: targetRef,
        restored: false,
        requires_manual_restore: true,
        restore_reason: "target_snapshot_content_missing",
        metadata,
      };
    }
    fs.mkdirSync(path.dirname(absoluteTargetPath), { recursive: true });
    fs.writeFileSync(absoluteTargetPath, fs.readFileSync(contentPath, "utf8"), "utf8");
    return {
      target_ref: targetRef,
      restored: true,
      requires_manual_restore: false,
      restore_reason: null,
      restore_action: "restore_file_content",
      metadata,
    };
  }
  return {
    target_ref: targetRef,
    restored: false,
    requires_manual_restore: true,
    restore_reason: "target_snapshot_requires_manual_restore",
    metadata,
  };
}

function updateSessionRecovery(rootDir, updater) {
  const session = readSession(rootDir);
  if (!session) {
    throw new Error("session.yaml is required for recovery updates");
  }
  const updated = updater(structuredClone(session));
  updated.timestamps = {
    ...updated.timestamps,
    updated_at: nowIso(),
  };
  writeSession(rootDir, updated);
  return updated;
}

function createCheckpoint(rootDir, options) {
  if (!options || !CHECKPOINT_CLASSES.has(options.checkpointClass)) {
    throw new Error("checkpointClass must be one of node|review|pre_destructive");
  }
  const session = readSession(rootDir);
  const taskGraph = readTaskGraph(rootDir);
  const sprintStatus = readSprintStatus(rootDir);
  if (!session || !taskGraph) {
    throw new Error("session.yaml and task-graph.yaml must exist before checkpoint creation");
  }
  ensureCheckpointDir(rootDir);
  const checkpointId = options.checkpointId || buildCheckpointId(options.checkpointClass);
  const sessionSnapshotRef = relativeCheckpointRef(checkpointId, "session.yaml");
  const taskGraphSnapshotRef = relativeCheckpointRef(checkpointId, "task-graph.yaml");
  const sprintStatusSnapshotRef = sprintStatus ? relativeCheckpointRef(checkpointId, "sprint-status.yaml") : null;
  persistYamlSnapshot(rootDir, sessionSnapshotRef, session);
  persistYamlSnapshot(rootDir, taskGraphSnapshotRef, taskGraph);
  if (sprintStatus && sprintStatusSnapshotRef) {
    persistYamlSnapshot(rootDir, sprintStatusSnapshotRef, sprintStatus);
  }
  const targetSnapshot = captureTargetSnapshot(rootDir, checkpointId, options);
  const capsuleSnapshot = captureLatestCapsuleSnapshot(rootDir, checkpointId);
  const gitHeadRef = readGitHeadRef(rootDir);
  const journalOffset = getJournalOffset(rootDir);
  const integrityHash = computeIntegrityHash([
    session,
    taskGraph,
    sprintStatusSnapshotRef,
    journalOffset,
    options.checkpointClass,
    options.phase,
    options.nodeId,
    targetSnapshot.target_snapshot_ref,
    capsuleSnapshot.capsule_snapshot_ref,
    gitHeadRef,
  ]);
  const checkpoint = {
    checkpoint_id: checkpointId,
    checkpoint_class: options.checkpointClass,
    run_id: session.run_id,
    phase: options.phase || session.phase.current,
    node_id: options.nodeId || session.node.active_id,
    session_snapshot_ref: sessionSnapshotRef,
    task_graph_snapshot_ref: taskGraphSnapshotRef,
    sprint_status_snapshot_ref: sprintStatusSnapshotRef,
    journal_offset: journalOffset,
    integrity_hash: integrityHash,
    restore_status: "not_restored",
    restore_verified_at: null,
    restore_source_event: options.sourceEvent || null,
    target_ref: options.targetRef || null,
    operation_kind: options.operationKind || null,
    command_class: options.commandClass || null,
    file_class: options.fileClass || null,
    metadata: clone(options.metadata || null),
    target_snapshot_ref: targetSnapshot.target_snapshot_ref,
    target_snapshot_content_ref: targetSnapshot.target_snapshot_content_ref,
    capsule_snapshot_ref: capsuleSnapshot.capsule_snapshot_ref,
    capsule_id: capsuleSnapshot.capsule_id,
    git_head_ref: gitHeadRef,
    created_at: nowIso(),
  };
  writeCheckpoint(rootDir, checkpoint);
  updateSessionRecovery(rootDir, (draft) => {
    draft.recovery.last_checkpoint_id = checkpointId;
    draft.recovery.restore_pending = false;
    draft.recovery.restore_reason = null;
    return draft;
  });
  appendEvent(rootDir, {
    runId: session.run_id,
    event: "checkpoint_created",
    phase: checkpoint.phase,
    nodeId: checkpoint.node_id,
    actor: options.actor || "runtime",
    payload: {
      checkpoint_id: checkpointId,
      checkpoint_class: options.checkpointClass,
      journal_offset: journalOffset,
      source_event: options.sourceEvent || null,
      target_ref: options.targetRef || null,
      operation_kind: options.operationKind || null,
      command_class: options.commandClass || null,
      file_class: options.fileClass || null,
      target_snapshot_ref: targetSnapshot.target_snapshot_ref,
      capsule_snapshot_ref: capsuleSnapshot.capsule_snapshot_ref,
      git_head_ref: gitHeadRef,
    },
  });
  return checkpoint;
}

function ensurePreDestructiveCheckpoint(rootDir, options = {}) {
  const session = readSession(rootDir);
  const taskGraph = readTaskGraph(rootDir);
  if (!session || !taskGraph) {
    return {
      created: false,
      skipped: true,
      reason: "runtime_state_missing",
      checkpoint: null,
    };
  }

  const nodeId = String(options.nodeId || session?.node?.active_id || "").trim();
  if (!nodeId || nodeId === "none") {
    return {
      created: false,
      skipped: true,
      reason: "active_node_missing",
      checkpoint: null,
    };
  }

  const targetRef = String(options.targetRef || "").trim();
  const lastCheckpointId = String(session?.recovery?.last_checkpoint_id || "").trim();
  if (lastCheckpointId) {
    const latestCheckpoint = readCheckpoint(rootDir, lastCheckpointId);
    if (
      latestCheckpoint
      && latestCheckpoint.run_id === session.run_id
      && latestCheckpoint.checkpoint_class === "pre_destructive"
      && latestCheckpoint.node_id === nodeId
      && String(latestCheckpoint.target_ref || "").trim() === targetRef
      && String(latestCheckpoint.operation_kind || "").trim() === String(options.operationKind || "").trim()
      && String(latestCheckpoint.command_class || "").trim() === String(options.commandClass || "").trim()
      && String(latestCheckpoint.file_class || "").trim() === String(options.fileClass || "").trim()
    ) {
      return {
        created: false,
        skipped: true,
        reason: "existing_pre_destructive_checkpoint",
        checkpoint: latestCheckpoint,
      };
    }
  }

  const checkpoint = createCheckpoint(rootDir, {
    checkpointClass: "pre_destructive",
    phase: options.phase || session.phase.current,
    nodeId,
    actor: options.actor || "runtime",
    sourceEvent: options.sourceEvent || "pre_destructive_guard",
    targetRef,
    operationKind: options.operationKind || null,
    commandClass: options.commandClass || null,
    fileClass: options.fileClass || null,
    metadata: options.metadata || null,
  });
  return {
    created: true,
    skipped: false,
    reason: null,
    checkpoint,
  };
}

function listCheckpoints(rootDir) {
  const dirPath = checkpointDir(rootDir);
  if (!fs.existsSync(dirPath)) {
    return [];
  }
  return fs
    .readdirSync(dirPath)
    .filter((fileName) => fileName.endsWith(".json"))
    .map((fileName) => JSON.parse(fs.readFileSync(path.join(dirPath, fileName), "utf8")))
    .sort((left, right) => {
      const leftKey = `${left.created_at}|${left.checkpoint_id}`;
      const rightKey = `${right.created_at}|${right.checkpoint_id}`;
      return rightKey.localeCompare(leftKey);
    });
}

function buildResumeFrontier(rootDir) {
  const session = readSession(rootDir);
  const sprintStatus = readSprintStatus(rootDir);
  const reasons = [];
  if (!sprintStatus || !Array.isArray(sprintStatus.recommended_next) || sprintStatus.recommended_next.length === 0) {
    reasons.push("resume_frontier_missing");
  }
  if (session?.recovery?.restore_pending) {
    reasons.push(session.recovery.restore_reason || "restore_pending");
  }
  return {
    recovery_required: reasons.length > 0,
    reasons,
    active_phase: sprintStatus?.active_phase || session?.phase?.current || null,
    active_node: session?.node?.active_id || null,
    recommended_next: sprintStatus?.recommended_next || [],
    pending_approval: session?.approvals?.pending || false,
    last_checkpoint_id: session?.recovery?.last_checkpoint_id || null,
  };
}

function hasMissingTerminalEvent(rootDir, nodeId) {
  const events = readEvents(rootDir);
  let latestStartIndex = -1;
  let latestTerminalIndex = -1;
  for (let index = 0; index < events.length; index += 1) {
    const event = events[index];
    if (event.node_id !== nodeId) {
      continue;
    }
    if (event.event === "node_started") {
      latestStartIndex = index;
    }
    if (TERMINAL_NODE_EVENTS.has(event.event)) {
      latestTerminalIndex = index;
    }
  }
  return latestStartIndex >= 0 && latestTerminalIndex < latestStartIndex;
}

function recoverInterruptedRun(rootDir, options = {}) {
  const session = readSession(rootDir);
  if (!session) {
    throw new Error("session.yaml is required for recovery");
  }
  const reasons = [];
  if (session.approvals?.pending) {
    reasons.push("pending_approval_survived_restart");
  }
  const nodeId = session.node?.active_id;
  if (nodeId && hasMissingTerminalEvent(rootDir, nodeId)) {
    reasons.push("missing_terminal_event");
    const latestFailure = findLatestEvent(rootDir, "validation_failed");
    const alreadyRecorded = latestFailure?.payload?.terminal_interpretation === "aborted"
      && latestFailure?.payload?.node_id === nodeId;
    if (!alreadyRecorded) {
      appendEvent(rootDir, {
        runId: session.run_id,
        event: "validation_failed",
        phase: session.phase.current,
        nodeId,
        actor: options.actor || "runtime",
        payload: {
          reason: "missing_terminal_event",
          node_id: nodeId,
          terminal_interpretation: "aborted",
        },
      });
    }
  }
  if (reasons.length > 0) {
    updateSessionRecovery(rootDir, (draft) => {
      draft.recovery.restore_pending = true;
      draft.recovery.restore_reason = reasons[0];
      return draft;
    });
  }
  const frontier = buildResumeFrontier(rootDir);
  return {
    ...frontier,
    recovery_required: frontier.recovery_required || reasons.length > 0,
    reasons: [...new Set([...reasons, ...frontier.reasons])],
  };
}

function restoreCheckpoint(rootDir, checkpointId, options = {}) {
  const checkpoint = readCheckpoint(rootDir, checkpointId);
  if (!checkpoint) {
    throw new Error(`checkpoint not found: ${checkpointId}`);
  }
  const sessionSnapshot = loadYamlFile(absoluteFromRelative(rootDir, checkpoint.session_snapshot_ref));
  const taskGraphSnapshot = loadYamlFile(absoluteFromRelative(rootDir, checkpoint.task_graph_snapshot_ref));
  const sprintStatusSnapshotRef = String(checkpoint?.sprint_status_snapshot_ref || "").trim();
  const sprintStatusSnapshot = sprintStatusSnapshotRef
    ? loadYamlFile(absoluteFromRelative(rootDir, sprintStatusSnapshotRef))
    : null;
  const restoredSession = structuredClone(sessionSnapshot);
  restoredSession.recovery = {
    ...restoredSession.recovery,
    last_checkpoint_id: checkpointId,
    restore_pending: false,
    restore_reason: null,
  };
  restoredSession.timestamps = {
    ...restoredSession.timestamps,
    updated_at: nowIso(),
  };
  writeSession(rootDir, restoredSession);
  writeTaskGraph(rootDir, taskGraphSnapshot);
  if (sprintStatusSnapshot) {
    writeSprintStatus(rootDir, sprintStatusSnapshot);
  }
  let restoredTarget = null;
  const restoredCapsule = restoreCapsuleSnapshot(rootDir, checkpoint);
  if (options.restoreTargetSnapshot === true || checkpoint.checkpoint_class === "pre_destructive") {
    restoredTarget = restoreTargetSnapshot(rootDir, checkpoint);
  }
  const manualRestoreReason = restoredTarget?.requires_manual_restore === true
    ? String(restoredTarget.restore_reason || "target_snapshot_requires_manual_restore")
    : null;
  if (manualRestoreReason) {
    updateSessionRecovery(rootDir, (draft) => {
      draft.recovery.last_checkpoint_id = checkpointId;
      draft.recovery.restore_pending = true;
      draft.recovery.restore_reason = manualRestoreReason;
      return draft;
    });
  }
  const restoredCheckpoint = {
    ...checkpoint,
    restore_status: manualRestoreReason ? "restore_failed" : "restored",
    restore_verified_at: nowIso(),
  };
  writeCheckpoint(rootDir, restoredCheckpoint);
  appendEvent(rootDir, {
    runId: restoredSession.run_id,
    event: "checkpoint_restored",
    phase: restoredCheckpoint.phase,
    nodeId: restoredCheckpoint.node_id,
    actor: options.actor || "runtime",
    payload: {
      checkpoint_id: checkpointId,
      restore_status: restoredCheckpoint.restore_status,
      restored_target_ref: restoredTarget?.target_ref || null,
      restored_capsule_id: restoredCapsule?.capsule_id || null,
      restore_reason: manualRestoreReason,
    },
  });
  if (manualRestoreReason) {
    appendEvent(rootDir, {
      runId: restoredSession.run_id,
      event: "validation_failed",
      phase: restoredCheckpoint.phase,
      nodeId: restoredCheckpoint.node_id,
      actor: options.actor || "runtime",
      payload: {
        reason: manualRestoreReason,
        checkpoint_id: checkpointId,
        target_ref: restoredTarget?.target_ref || null,
        requires_human: true,
      },
    });
  }
  refreshLedger(rootDir);
  return restoredCheckpoint;
}

module.exports = {
  buildResumeFrontier,
  createCheckpoint,
  ensurePreDestructiveCheckpoint,
  listCheckpoints,
  recoverInterruptedRun,
  restoreCheckpoint,
};
