"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { dumpYamlFile, loadYamlFile } = require("./yaml-loader.cjs");

const RUNTIME_LAYOUT = {
  root: [".ai", "workflow"],
  session: [".ai", "workflow", "session.yaml"],
  taskGraph: [".ai", "workflow", "task-graph.yaml"],
  sprintStatus: [".ai", "workflow", "sprint-status.yaml"],
  journal: [".ai", "workflow", "journal.jsonl"],
  ledger: [".ai", "workflow", "ledger.md"],
  capsules: [".ai", "workflow", "capsules"],
  checkpoints: [".ai", "workflow", "checkpoints"],
};

const JOURNAL_MAX_LINE_BYTES = 4096;
const JOURNAL_LOCK_RETRY_MS = 20;
const JOURNAL_LOCK_MAX_WAIT_MS = 10000;
const JOURNAL_LOCK_STALE_MS = 5000;
const JOURNAL_LOCK_MAX_RETRIES = Math.ceil(JOURNAL_LOCK_MAX_WAIT_MS / JOURNAL_LOCK_RETRY_MS);

function toArray(value) {
  return Array.isArray(value) ? value : [];
}

function resolveRoot(rootDir) {
  return path.resolve(rootDir);
}

function resolvePath(rootDir, segments) {
  return path.join(resolveRoot(rootDir), ...segments);
}

function ensureRuntimeLayout(rootDir) {
  fs.mkdirSync(resolvePath(rootDir, RUNTIME_LAYOUT.root), { recursive: true });
  fs.mkdirSync(resolvePath(rootDir, RUNTIME_LAYOUT.capsules), { recursive: true });
  fs.mkdirSync(resolvePath(rootDir, RUNTIME_LAYOUT.checkpoints), { recursive: true });
}

function atomicWriteText(filePath, content, options = {}) {
  const absolutePath = path.resolve(filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  const tempPath = `${absolutePath}.tmp-${process.pid}-${Date.now()}`;
  try {
    fs.writeFileSync(tempPath, content, "utf8");
    if (options.injectFailure === "before_commit") {
      throw new Error(`Injected failure before commit for ${absolutePath}`);
    }
    fs.renameSync(tempPath, absolutePath);
  } catch (error) {
    if (fs.existsSync(tempPath)) {
      fs.rmSync(tempPath, { force: true });
    }
    throw error;
  }
}

function atomicWriteYaml(filePath, value, options = {}) {
  const absolutePath = path.resolve(filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  const tempPath = `${absolutePath}.tmp-${process.pid}-${Date.now()}.yaml`;
  try {
    dumpYamlFile(tempPath, value);
    if (options.injectFailure === "before_commit") {
      throw new Error(`Injected failure before commit for ${absolutePath}`);
    }
    fs.renameSync(tempPath, absolutePath);
  } catch (error) {
    if (fs.existsSync(tempPath)) {
      fs.rmSync(tempPath, { force: true });
    }
    throw error;
  }
}

function readYamlAsset(filePath) {
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return loadYamlFile(filePath);
}

function readSession(rootDir) {
  return readYamlAsset(resolvePath(rootDir, RUNTIME_LAYOUT.session));
}

function writeSession(rootDir, value, options = {}) {
  ensureRuntimeLayout(rootDir);
  atomicWriteYaml(resolvePath(rootDir, RUNTIME_LAYOUT.session), value, options);
}

function readTaskGraph(rootDir) {
  return readYamlAsset(resolvePath(rootDir, RUNTIME_LAYOUT.taskGraph));
}

function writeTaskGraph(rootDir, value, options = {}) {
  ensureRuntimeLayout(rootDir);
  atomicWriteYaml(resolvePath(rootDir, RUNTIME_LAYOUT.taskGraph), value, options);
}

function readSprintStatus(rootDir) {
  return readYamlAsset(resolvePath(rootDir, RUNTIME_LAYOUT.sprintStatus));
}

function writeSprintStatus(rootDir, value, options = {}) {
  ensureRuntimeLayout(rootDir);
  atomicWriteYaml(resolvePath(rootDir, RUNTIME_LAYOUT.sprintStatus), value, options);
}

function readJournalEvents(rootDir) {
  const journalPath = resolvePath(rootDir, RUNTIME_LAYOUT.journal);
  if (!fs.existsSync(journalPath)) {
    return [];
  }
  return fs
    .readFileSync(journalPath, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

function sleepSync(durationMs) {
  const buffer = new SharedArrayBuffer(4);
  const view = new Int32Array(buffer);
  Atomics.wait(view, 0, 0, durationMs);
}

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) {
    return false;
  }
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error && error.code === "ESRCH") {
      return false;
    }
    return true;
  }
}

function readJournalLockMetadata(lockPath) {
  try {
    const raw = fs.readFileSync(lockPath, "utf8").trim();
    if (!raw) {
      return null;
    }
    return JSON.parse(raw);
  } catch (error) {
    return null;
  }
}

function isJournalLockStale(lockPath) {
  const info = readJournalLockMetadata(lockPath);
  if (info && typeof info.created_at === "string") {
    const createdAtMs = Date.parse(info.created_at);
    if (Number.isFinite(createdAtMs) && Date.now() - createdAtMs > JOURNAL_LOCK_STALE_MS) {
      return true;
    }
  }
  if (info && typeof info.pid === "number") {
    return !isProcessAlive(info.pid);
  }
  try {
    const stat = fs.statSync(lockPath);
    return Date.now() - stat.mtimeMs > JOURNAL_LOCK_STALE_MS;
  } catch (error) {
    if (error && error.code === "ENOENT") {
      return false;
    }
    throw error;
  }
}


function acquireJournalLock(lockPath) {
  const deadline = Date.now() + JOURNAL_LOCK_MAX_WAIT_MS;
  for (let attempt = 0; attempt < JOURNAL_LOCK_MAX_RETRIES; attempt += 1) {
    try {
      const lockFd = fs.openSync(lockPath, "wx");
      try {
        const payload = JSON.stringify({ pid: process.pid, created_at: new Date().toISOString() });
        fs.writeSync(lockFd, `${payload}\n`);
      } catch (error) {
        fs.closeSync(lockFd);
        throw error;
      }
      return lockFd;
    } catch (error) {
      if (error && error.code !== "EEXIST") {
        throw error;
      }
      if (isJournalLockStale(lockPath)) {
        try {
          fs.rmSync(lockPath, { force: true });
        } catch (removeError) {
          if (removeError && removeError.code !== "ENOENT") {
            throw removeError;
          }
        }
        continue;
      }
      if (Date.now() >= deadline) {
        break;
      }
      sleepSync(JOURNAL_LOCK_RETRY_MS);
    }
  }
  throw new Error(`JOURNAL_LOCK_TIMEOUT ${lockPath}`);
}


function releaseJournalLock(lockFd, lockPath) {
  if (typeof lockFd === "number") {
    fs.closeSync(lockFd);
  }
  if (fs.existsSync(lockPath)) {
    fs.rmSync(lockPath, { force: true });
  }
}

function appendJournalEvents(rootDir, events) {
  ensureRuntimeLayout(rootDir);
  const journalPath = resolvePath(rootDir, RUNTIME_LAYOUT.journal);
  const payloads = toArray(events).map((event) => JSON.stringify(event));
  if (payloads.length === 0) {
    return;
  }
  for (const payload of payloads) {
    if (Buffer.byteLength(payload, "utf8") > JOURNAL_MAX_LINE_BYTES) {
      throw new Error(`JOURNAL_LINE_TOO_LARGE ${payload.slice(0, 64)}...`);
    }
  }
  const chunk = `${payloads.join("\n")}\n`;
  if (process.platform === "win32") {
    const lockPath = `${journalPath}.lock`;
    const lockFd = acquireJournalLock(lockPath);
    try {
      fs.writeFileSync(journalPath, chunk, { flag: "a" });
    } finally {
      releaseJournalLock(lockFd, lockPath);
    }
    return;
  }
  fs.writeFileSync(journalPath, chunk, { flag: "a" });
}

function readLedger(rootDir) {
  const ledgerPath = resolvePath(rootDir, RUNTIME_LAYOUT.ledger);
  if (!fs.existsSync(ledgerPath)) {
    return null;
  }
  return fs.readFileSync(ledgerPath, "utf8");
}

function writeLedger(rootDir, content, options = {}) {
  ensureRuntimeLayout(rootDir);
  atomicWriteText(resolvePath(rootDir, RUNTIME_LAYOUT.ledger), content, options);
}

function resolveCheckpointPath(rootDir, checkpointId) {
  return path.join(resolvePath(rootDir, RUNTIME_LAYOUT.checkpoints), `${checkpointId}.json`);
}

function resolveCapsulePath(rootDir, capsuleId) {
  return path.join(resolvePath(rootDir, RUNTIME_LAYOUT.capsules), `${capsuleId}.json`);
}

function writeCapsule(rootDir, capsule, options = {}) {
  ensureRuntimeLayout(rootDir);
  if (!capsule || typeof capsule.capsule_id !== "string" || capsule.capsule_id.length === 0) {
    throw new Error("capsule.capsule_id is required");
  }
  atomicWriteText(
    resolveCapsulePath(rootDir, capsule.capsule_id),
    `${JSON.stringify(capsule, null, 2)}\n`,
    options,
  );
}

function readCapsule(rootDir, capsuleId) {
  const filePath = resolveCapsulePath(rootDir, capsuleId);
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function listCapsules(rootDir) {
  const dirPath = resolvePath(rootDir, RUNTIME_LAYOUT.capsules);
  if (!fs.existsSync(dirPath)) {
    return [];
  }
  return fs
    .readdirSync(dirPath)
    .filter((fileName) => fileName.endsWith(".json"))
    .map((fileName) => JSON.parse(fs.readFileSync(path.join(dirPath, fileName), "utf8")))
    .sort((left, right) => {
      const leftKey = `${left.created_at || ""}|${left.capsule_id || ""}`;
      const rightKey = `${right.created_at || ""}|${right.capsule_id || ""}`;
      return rightKey.localeCompare(leftKey);
    });
}

function writeCheckpoint(rootDir, checkpoint, options = {}) {
  ensureRuntimeLayout(rootDir);
  if (!checkpoint || typeof checkpoint.checkpoint_id !== "string" || checkpoint.checkpoint_id.length === 0) {
    throw new Error("checkpoint.checkpoint_id is required");
  }
  atomicWriteText(
    resolveCheckpointPath(rootDir, checkpoint.checkpoint_id),
    `${JSON.stringify(checkpoint, null, 2)}\n`,
    options,
  );
}

function readCheckpoint(rootDir, checkpointId) {
  const filePath = resolveCheckpointPath(rootDir, checkpointId);
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

module.exports = {
  appendJournalEvents,
  ensureRuntimeLayout,
  listCapsules,
  readCapsule,
  readCheckpoint,
  readJournalEvents,
  readLedger,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeCapsule,
  writeCheckpoint,
  writeLedger,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
};
