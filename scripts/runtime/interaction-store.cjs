"use strict";

// interaction-store.cjs — P1-N1: Durable Interaction Store
//
// Manages .ai/workflow/interactions/ layout:
//   requests/ix-*.json   — pending/active request objects
//   responses/ix-*.json  — presenter response objects
//   archive/ix-*.json    — closed interaction snapshots
//   active.json          — current active interaction index

const fs = require("node:fs");
const path = require("node:path");

// ─── Layout paths ────────────────────────────────────────────────────────────

function interactionsRoot(rootDir) {
  return path.join(path.resolve(rootDir), ".ai", "workflow", "interactions");
}

function requestsDir(rootDir) {
  return path.join(interactionsRoot(rootDir), "requests");
}

function responsesDir(rootDir) {
  return path.join(interactionsRoot(rootDir), "responses");
}

function archiveDir(rootDir) {
  return path.join(interactionsRoot(rootDir), "archive");
}

function activePath(rootDir) {
  return path.join(interactionsRoot(rootDir), "active.json");
}

function requestPath(rootDir, interactionId) {
  return path.join(requestsDir(rootDir), `${interactionId}.json`);
}

function responsePath(rootDir, interactionId) {
  return path.join(responsesDir(rootDir), `${interactionId}.json`);
}

function archivePath(rootDir, interactionId) {
  return path.join(archiveDir(rootDir), `${interactionId}.json`);
}

// ─── Atomic write helper ─────────────────────────────────────────────────────
// Reuses the same pattern as store.cjs: write to .tmp then rename

function atomicWriteJson(filePath, obj) {
  const absolutePath = path.resolve(filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  const tempPath = `${absolutePath}.tmp-${process.pid}-${Date.now()}`;
  try {
    fs.writeFileSync(tempPath, JSON.stringify(obj, null, 2) + "\n", "utf8");
    fs.renameSync(tempPath, absolutePath);
  } catch (error) {
    if (fs.existsSync(tempPath)) {
      fs.rmSync(tempPath, { force: true });
    }
    throw error;
  }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/**
 * Create the interactions directory layout under rootDir.
 * Idempotent — safe to call multiple times.
 */
function ensureInteractionLayout(rootDir) {
  fs.mkdirSync(requestsDir(rootDir), { recursive: true });
  fs.mkdirSync(responsesDir(rootDir), { recursive: true });
  fs.mkdirSync(archiveDir(rootDir), { recursive: true });
}

/**
 * Atomically write a request object to requests/ix-*.json.
 * @param {string} rootDir
 * @param {object} requestObj — must have .interaction_id
 */
function writeRequest(rootDir, requestObj) {
  if (!requestObj || typeof requestObj.interaction_id !== "string") {
    throw new Error("requestObj.interaction_id is required");
  }
  ensureInteractionLayout(rootDir);
  atomicWriteJson(requestPath(rootDir, requestObj.interaction_id), requestObj);
}

/**
 * Read a request object from requests/ix-*.json.
 * Returns null if the file does not exist.
 */
function readRequest(rootDir, interactionId) {
  const filePath = requestPath(rootDir, interactionId);
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

/**
 * Atomically write a response object to responses/ix-*.json.
 * @param {string} rootDir
 * @param {object} responseObj — must have .interaction_id
 */
function writeResponse(rootDir, responseObj) {
  if (!responseObj || typeof responseObj.interaction_id !== "string") {
    throw new Error("responseObj.interaction_id is required");
  }
  ensureInteractionLayout(rootDir);
  atomicWriteJson(responsePath(rootDir, responseObj.interaction_id), responseObj);
}

/**
 * Read a response object from responses/ix-*.json.
 * Returns null if the file does not exist.
 */
function readResponse(rootDir, interactionId) {
  const filePath = responsePath(rootDir, interactionId);
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

/**
 * Read active.json. Returns null if not present.
 */
function getActive(rootDir) {
  const filePath = activePath(rootDir);
  if (!fs.existsSync(filePath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

/**
 * Atomically write active.json.
 * activeObj must have: active_id, pending_count, blocking_kind, blocking_reason, created_at
 */
function setActive(rootDir, activeObj) {
  if (!activeObj || typeof activeObj.active_id !== "string") {
    throw new Error("activeObj.active_id is required");
  }
  ensureInteractionLayout(rootDir);
  atomicWriteJson(activePath(rootDir), activeObj);
}

/**
 * Remove or null out active.json.
 */
function clearActive(rootDir) {
  const filePath = activePath(rootDir);
  if (fs.existsSync(filePath)) {
    fs.rmSync(filePath, { force: true });
  }
}

/**
 * Move request + response files to archive/.
 * Produces a single archive snapshot with both merged.
 * Missing response is tolerated.
 */
function archiveInteraction(rootDir, interactionId) {
  const reqFile = requestPath(rootDir, interactionId);
  const respFile = responsePath(rootDir, interactionId);
  const archFile = archivePath(rootDir, interactionId);

  ensureInteractionLayout(rootDir);

  const requestObj = fs.existsSync(reqFile)
    ? JSON.parse(fs.readFileSync(reqFile, "utf8"))
    : null;
  const responseObj = fs.existsSync(respFile)
    ? JSON.parse(fs.readFileSync(respFile, "utf8"))
    : null;

  const snapshot = {
    archived_at: new Date().toISOString(),
    request: requestObj,
    response: responseObj,
  };
  atomicWriteJson(archFile, snapshot);

  // Remove from active directories
  if (fs.existsSync(reqFile)) {
    fs.rmSync(reqFile, { force: true });
  }
  if (fs.existsSync(respFile)) {
    fs.rmSync(respFile, { force: true });
  }
}

/**
 * Return array of all request objects in requests/ with status="pending".
 */
function listPending(rootDir) {
  const dir = requestsDir(rootDir);
  if (!fs.existsSync(dir)) {
    return [];
  }
  const files = fs.readdirSync(dir).filter((f) => f.endsWith(".json"));
  const result = [];
  for (const file of files) {
    const filePath = path.join(dir, file);
    try {
      const obj = JSON.parse(fs.readFileSync(filePath, "utf8"));
      if (obj.status === "pending") {
        result.push(obj);
      }
    } catch {
      // skip malformed files
    }
  }
  return result;
}

module.exports = {
  archiveInteraction,
  clearActive,
  ensureInteractionLayout,
  getActive,
  listPending,
  readRequest,
  readResponse,
  setActive,
  writeRequest,
  writeResponse,
};
