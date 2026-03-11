"use strict";

const fs = require("node:fs");
const path = require("node:path");

const OUTPUT_LOG_RELATIVE = [".ai", "workflow", "output.log"];

function resolveRoot(rootDir) {
  return path.resolve(rootDir || path.join(__dirname, "..", ".."));
}

function resolveOutputLogPath(rootDir) {
  return path.join(resolveRoot(rootDir), ...OUTPUT_LOG_RELATIVE);
}

function ensureOutputLogDir(rootDir) {
  const logPath = resolveOutputLogPath(rootDir);
  fs.mkdirSync(path.dirname(logPath), { recursive: true });
  return logPath;
}

function appendOutputLog(rootDir, entry) {
  if (!entry || typeof entry !== "object") {
    throw new Error("output log entry must be an object");
  }
  const logPath = ensureOutputLogDir(rootDir);
  const payload = JSON.stringify(entry);
  fs.writeFileSync(logPath, `${payload}\n`, { flag: "a" });
  return logPath;
}

function appendOutputLogs(rootDir, entries) {
  const list = Array.isArray(entries) ? entries : [];
  if (list.length === 0) {
    return null;
  }
  const logPath = ensureOutputLogDir(rootDir);
  const payload = list.map((entry) => JSON.stringify(entry)).join("\n");
  fs.writeFileSync(logPath, `${payload}\n`, { flag: "a" });
  return logPath;
}

function readOutputLog(rootDir) {
  const logPath = resolveOutputLogPath(rootDir);
  if (!fs.existsSync(logPath)) {
    return [];
  }
  return fs
    .readFileSync(logPath, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

module.exports = {
  appendOutputLog,
  appendOutputLogs,
  readOutputLog,
  resolveOutputLogPath,
};
