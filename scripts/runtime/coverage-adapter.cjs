#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { getNodeById } = require("./runtime-state.cjs");
const { readSession, readTaskGraph } = require("./store.cjs");

const COVERAGE_STAGING_RELATIVE_PATH = ".ai/analysis/coverage-staging.json";

const COVERAGE_TARGETS = {
  critical: 100,
  core: 90,
  standard: 80,
  utility: 60,
  scaffold: null,
};

const DISCOVERY_CANDIDATES = [
  { relativePath: "coverage/coverage-summary.json", format: "istanbul_summary" },
  { relativePath: "coverage-summary.json", format: "istanbul_summary" },
  { relativePath: "coverage/coverage.xml", format: "cobertura_xml" },
  { relativePath: "coverage.xml", format: "cobertura_xml" },
];

function nowIso() {
  return new Date().toISOString();
}

function toPosixPath(value) {
  return String(value || "").replace(/\\/g, "/");
}

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function readJsonIfExists(filePath) {
  try {
    if (!fs.existsSync(filePath)) {
      return null;
    }
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch {
    return null;
  }
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
  fs.writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
}

function normalizeBoolean(value, fallback = false) {
  if (typeof value === "boolean") {
    return value;
  }
  if (typeof value === "string") {
    if (value === "true") return true;
    if (value === "false") return false;
  }
  return fallback;
}

function normalizeNumber(value) {
  const numeric = Number(value);
  return Number.isFinite(numeric) ? numeric : null;
}

function resolveCoverageContract({ session, node, overrides = {} }) {
  const contract = overrides.testContract || node?.test_contract || {};
  return {
    coverage_mode: overrides.coverage_mode || contract.coverage_mode || "full",
    coverage_profile: overrides.coverage_profile || contract.coverage_profile || "standard",
    required: COVERAGE_TARGETS[overrides.coverage_profile || contract.coverage_profile] ?? null,
    task_mode: overrides.task_mode || session?.task?.mode || "feature",
  };
}

function detectInputFormat({ inputPath, rawContent, parsedJson }) {
  if (parsedJson && parsedJson.adapter_kind === "coverage_adapter" && Number(parsedJson.schema_version) === 1) {
    return "normalized_json";
  }
  if (
    parsedJson &&
    parsedJson.total &&
    parsedJson.total.lines &&
    Number.isFinite(Number(parsedJson.total.lines.pct))
  ) {
    return "istanbul_summary";
  }
  const loweredPath = String(inputPath || "").toLowerCase();
  if (loweredPath.endsWith(".xml") || String(rawContent || "").trim().startsWith("<")) {
    return "cobertura_xml";
  }
  return "unknown";
}

function parseIstanbulSummary(summary) {
  const total = summary?.total || {};
  const lines = normalizeNumber(total?.lines?.pct);
  const statements = normalizeNumber(total?.statements?.pct);
  const functions = normalizeNumber(total?.functions?.pct);
  const branches = normalizeNumber(total?.branches?.pct);
  const actual = lines ?? statements ?? functions ?? branches;
  return {
    actual,
    actual_basis: lines !== null ? "lines_pct" : statements !== null ? "statements_pct" : functions !== null ? "functions_pct" : "branches_pct",
    metrics: {
      lines_pct: lines,
      statements_pct: statements,
      functions_pct: functions,
      branches_pct: branches,
    },
  };
}

function parseCoberturaXml(xmlText) {
  const lineRateMatch = xmlText.match(/line-rate="([0-9.]+)"/i);
  const branchRateMatch = xmlText.match(/branch-rate="([0-9.]+)"/i);
  const linesCoveredMatch = xmlText.match(/lines-covered="([0-9.]+)"/i);
  const linesValidMatch = xmlText.match(/lines-valid="([0-9.]+)"/i);

  let linesPct = null;
  if (lineRateMatch) {
    linesPct = Number(lineRateMatch[1]) * 100;
  } else if (linesCoveredMatch && linesValidMatch && Number(linesValidMatch[1]) > 0) {
    linesPct = (Number(linesCoveredMatch[1]) / Number(linesValidMatch[1])) * 100;
  }

  const branchPct = branchRateMatch ? Number(branchRateMatch[1]) * 100 : null;
  const actual = Number.isFinite(linesPct) ? linesPct : branchPct;
  return {
    actual,
    actual_basis: Number.isFinite(linesPct) ? "lines_pct" : "branches_pct",
    metrics: {
      lines_pct: Number.isFinite(linesPct) ? Number(linesPct.toFixed(2)) : null,
      statements_pct: null,
      functions_pct: null,
      branches_pct: Number.isFinite(branchPct) ? Number(branchPct.toFixed(2)) : null,
    },
  };
}

function parseNormalizedJson(input) {
  return {
    actual: normalizeNumber(input.actual),
    actual_basis: String(input.actual_basis || "lines_pct"),
    metrics: clone(input.metrics || {
      lines_pct: normalizeNumber(input.actual),
      statements_pct: null,
      functions_pct: null,
      branches_pct: null,
    }),
  };
}

function computeCoverageVerdict({ actual, required, coverage_mode, task_mode, globalRegressed, touchedRegionRegressed, characterizationAdded }) {
  if (required === null) {
    return { pass: true, status: "n/a", reasons: [] };
  }
  if (!Number.isFinite(actual)) {
    return { pass: false, status: "fail", reasons: ["coverage_actual_missing"] };
  }
  const thresholdPass = actual >= required;
  if (coverage_mode === "patch") {
    const reasons = [];
    if (!thresholdPass) reasons.push("patch_threshold_not_met");
    if (globalRegressed === true) reasons.push("global_coverage_regressed");
    if (touchedRegionRegressed === true) reasons.push("touched_region_coverage_regressed");
    if (task_mode === "bugfix" && characterizationAdded !== true) reasons.push("characterization_test_missing");
    return {
      pass: reasons.length === 0,
      status: reasons.length === 0 ? "pass" : "fail",
      reasons,
    };
  }
  return {
    pass: thresholdPass,
    status: thresholdPass ? "pass" : "fail",
    reasons: thresholdPass ? [] : ["coverage_below_threshold"],
  };
}

function discoverCoverageInput(rootDir) {
  const resolvedRoot = path.resolve(rootDir);
  for (const candidate of DISCOVERY_CANDIDATES) {
    const absolutePath = path.join(resolvedRoot, candidate.relativePath);
    if (fs.existsSync(absolutePath)) {
      return {
        absolutePath,
        relativePath: candidate.relativePath,
        format: candidate.format,
      };
    }
  }
  return null;
}

function resolveActiveNode(rootDir, session, taskGraph) {
  const activeId = session?.node?.active_id;
  if (typeof activeId === "string" && activeId && activeId !== "none") {
    return getNodeById(taskGraph, activeId);
  }
  return Array.isArray(taskGraph?.nodes) ? taskGraph.nodes[0] || null : null;
}

function buildCoverageEvidence(options = {}) {
  const rootDir = path.resolve(options.rootDir || path.join(__dirname, "..", ".."));
  const session = options.session || readSession(rootDir) || {};
  const taskGraph = options.taskGraph || readTaskGraph(rootDir) || {};
  const node = options.node || resolveActiveNode(rootDir, session, taskGraph);
  const overrides = options.overrides || {};
  const contract = resolveCoverageContract({ session, node, overrides });

  let sourceRef = null;
  let sourceFormat = options.format || "unknown";
  let rawContent = options.rawContent || null;
  let parsedJson = options.parsedJson || null;

  if (!rawContent && options.inputPath) {
    const absoluteInputPath = path.resolve(rootDir, options.inputPath);
    sourceRef = toPosixPath(path.relative(rootDir, absoluteInputPath)) || toPosixPath(options.inputPath);
    rawContent = fs.readFileSync(absoluteInputPath, "utf8");
    try {
      parsedJson = JSON.parse(rawContent);
    } catch {
      parsedJson = null;
    }
    if (sourceFormat === "auto" || sourceFormat === "unknown") {
      sourceFormat = detectInputFormat({ inputPath: absoluteInputPath, rawContent, parsedJson });
    }
  }

  if (sourceFormat === "auto" || sourceFormat === "unknown") {
    sourceFormat = detectInputFormat({ inputPath: options.inputPath, rawContent, parsedJson });
  }

  let parsedMetrics = { actual: null, actual_basis: null, metrics: { lines_pct: null, statements_pct: null, functions_pct: null, branches_pct: null } };

  if (sourceFormat === "normalized_json" && parsedJson) {
    parsedMetrics = parseNormalizedJson(parsedJson);
  } else if (sourceFormat === "istanbul_summary" && parsedJson) {
    parsedMetrics = parseIstanbulSummary(parsedJson);
  } else if (sourceFormat === "cobertura_xml" && rawContent) {
    parsedMetrics = parseCoberturaXml(rawContent);
  }

  const normalizedFlags = {
    globalRegressed: normalizeBoolean(overrides.globalRegressed, normalizeBoolean(parsedJson?.globalRegressed, false)),
    touchedRegionRegressed: normalizeBoolean(overrides.touchedRegionRegressed, normalizeBoolean(parsedJson?.touchedRegionRegressed, false)),
    characterizationAdded: normalizeBoolean(overrides.characterizationAdded, normalizeBoolean(parsedJson?.characterizationAdded, false)),
  };

  const verdict = computeCoverageVerdict({
    actual: parsedMetrics.actual,
    required: contract.required,
    coverage_mode: contract.coverage_mode,
    task_mode: contract.task_mode,
    globalRegressed: normalizedFlags.globalRegressed,
    touchedRegionRegressed: normalizedFlags.touchedRegionRegressed,
    characterizationAdded: normalizedFlags.characterizationAdded,
  });

  return {
    schema_version: 1,
    adapter_kind: "coverage_adapter",
    source_format: sourceFormat,
    source_ref: sourceRef || (parsedJson?.source_ref ? String(parsedJson.source_ref) : null),
    session_run_id: session?.run_id || null,
    node_id: node?.id || null,
    task_mode: contract.task_mode,
    coverage_mode: contract.coverage_mode,
    coverage_profile: contract.coverage_profile,
    required: contract.required,
    actual: Number.isFinite(parsedMetrics.actual) ? Number(parsedMetrics.actual.toFixed(2)) : null,
    actual_basis: parsedMetrics.actual_basis,
    metrics: parsedMetrics.metrics,
    globalRegressed: normalizedFlags.globalRegressed,
    touchedRegionRegressed: normalizedFlags.touchedRegionRegressed,
    characterizationAdded: normalizedFlags.characterizationAdded,
    pass: verdict.pass,
    status: verdict.status,
    reasons: verdict.reasons,
    recorded_at: nowIso(),
  };
}

function validateCoverageEvidenceEnvelope(coverageEvidence) {
  if (!coverageEvidence || typeof coverageEvidence !== "object") {
    return { valid: false, reason: "coverage_evidence_missing" };
  }
  if (Number(coverageEvidence.schema_version) !== 1 || coverageEvidence.adapter_kind !== "coverage_adapter") {
    return { valid: false, reason: "coverage_evidence_invalid" };
  }
  if (typeof coverageEvidence.coverage_mode !== "string" || typeof coverageEvidence.coverage_profile !== "string") {
    return { valid: false, reason: "coverage_evidence_invalid" };
  }
  if (typeof coverageEvidence.pass !== "boolean" || typeof coverageEvidence.status !== "string" || !Array.isArray(coverageEvidence.reasons)) {
    return { valid: false, reason: "coverage_evidence_invalid" };
  }
  return { valid: true, reason: null };
}

function loadCoverageEvidence(rootDir, options = {}) {
  const resolvedRoot = path.resolve(rootDir);
  const stagingPath = path.join(resolvedRoot, COVERAGE_STAGING_RELATIVE_PATH);
  const staged = readJsonIfExists(stagingPath);
  if (staged && validateCoverageEvidenceEnvelope(staged).valid) {
    return staged;
  }
  if (options.autoDiscover !== true) {
    return null;
  }
  const discovered = discoverCoverageInput(resolvedRoot);
  if (!discovered) {
    return null;
  }
  return buildCoverageEvidence({
    rootDir: resolvedRoot,
    inputPath: discovered.relativePath,
    format: discovered.format,
    overrides: options.overrides || {},
  });
}

function writeCoverageEvidence(rootDir, coverageEvidence) {
  const resolvedRoot = path.resolve(rootDir);
  const stagingPath = path.join(resolvedRoot, COVERAGE_STAGING_RELATIVE_PATH);
  writeJson(stagingPath, coverageEvidence);
  return {
    staging_ref: COVERAGE_STAGING_RELATIVE_PATH,
    staging_path: stagingPath,
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    inputPath: null,
    format: "auto",
    write: false,
    json: false,
    overrides: {},
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--input":
        index += 1;
        parsed.inputPath = argv[index];
        break;
      case "--format":
        index += 1;
        parsed.format = argv[index];
        break;
      case "--coverage-mode":
        index += 1;
        parsed.overrides.coverage_mode = argv[index];
        break;
      case "--coverage-profile":
        index += 1;
        parsed.overrides.coverage_profile = argv[index];
        break;
      case "--global-regressed":
        index += 1;
        parsed.overrides.globalRegressed = argv[index];
        break;
      case "--touched-region-regressed":
        index += 1;
        parsed.overrides.touchedRegionRegressed = argv[index];
        break;
      case "--characterization-added":
        index += 1;
        parsed.overrides.characterizationAdded = argv[index];
        break;
      case "--write":
        parsed.write = true;
        break;
      case "--json":
        parsed.json = true;
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
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
    const discovered = !parsed.inputPath ? discoverCoverageInput(parsed.rootDir) : null;
    if (!parsed.inputPath && !discovered) {
      throw new Error("COVERAGE_INPUT_NOT_FOUND no supported coverage artifact was discovered under the workspace root");
    }
    const evidence = buildCoverageEvidence({
      rootDir: parsed.rootDir,
      inputPath: parsed.inputPath || discovered?.relativePath || null,
      format: parsed.inputPath ? parsed.format : discovered?.format || parsed.format,
      overrides: parsed.overrides,
    });
    const writeResult = parsed.write ? writeCoverageEvidence(parsed.rootDir, evidence) : null;
    const output = {
      ...evidence,
      staging_ref: writeResult?.staging_ref || null,
    };
    if (parsed.json) {
      console.log(JSON.stringify(output, null, 2));
      return;
    }
    console.log(`[coverage-adapter] status=${output.status} actual=${output.actual ?? "n/a"} required=${output.required ?? "n/a"}`);
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  COVERAGE_STAGING_RELATIVE_PATH,
  COVERAGE_TARGETS,
  buildCoverageEvidence,
  discoverCoverageInput,
  loadCoverageEvidence,
  parseArgs,
  validateCoverageEvidenceEnvelope,
  writeCoverageEvidence,
};

if (require.main === module) {
  main();
}
