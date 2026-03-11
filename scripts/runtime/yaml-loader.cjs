"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const PYTHON_YAML_LOAD = [
  "import json",
  "import pathlib",
  "import sys",
  "import yaml",
  "spec_path = pathlib.Path(sys.argv[1])",
  "with spec_path.open('r', encoding='utf-8') as handle:",
  "    data = yaml.safe_load(handle)",
  "print(json.dumps(data, ensure_ascii=False))",
].join("\n");

const PYTHON_YAML_DUMP = [
  "import json",
  "import pathlib",
  "import sys",
  "import yaml",
  "target_path = pathlib.Path(sys.argv[1])",
  "payload = json.loads(sys.stdin.read())",
  "with target_path.open('w', encoding='utf-8') as handle:",
  "    yaml.safe_dump(payload, handle, allow_unicode=True, sort_keys=False)",
].join("\n");

function resolvePythonCandidates() {
  const candidates = [];
  if (process.env.PYTHON) {
    candidates.push(process.env.PYTHON);
  }
  candidates.push("python");
  if (process.platform !== "win32") {
    candidates.push("python3");
  }
  return [...new Set(candidates)];
}

function runPythonBridge(args, options = {}) {
  let lastError = null;
  for (const candidate of resolvePythonCandidates()) {
    const result = spawnSync(candidate, args, {
      encoding: "utf8",
      input: options.input,
    });
    if (result.error) {
      lastError = result.error;
      continue;
    }
    if (result.status !== 0) {
      lastError = new Error(String(result.stderr || result.stdout || "Python bridge failed").trim());
      continue;
    }
    return result;
  }
  throw new Error(lastError ? lastError.message : "Python bridge unavailable");
}

function loadYamlFile(filePath) {
  const absolutePath = path.resolve(filePath);
  if (!fs.existsSync(absolutePath)) {
    throw new Error(`Spec file not found: ${absolutePath}`);
  }
  const result = runPythonBridge(["-c", PYTHON_YAML_LOAD, absolutePath]);
  return JSON.parse(String(result.stdout || "null"));
}

function dumpYamlFile(filePath, value) {
  const absolutePath = path.resolve(filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  runPythonBridge(["-c", PYTHON_YAML_DUMP, absolutePath], {
    input: JSON.stringify(value),
  });
}

module.exports = {
  dumpYamlFile,
  loadYamlFile,
};
