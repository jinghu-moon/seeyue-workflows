#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { loadCoverageEvidence } = require("./coverage-adapter.cjs");
const { readJournalEvents, readSession, readTaskGraph } = require("./store.cjs");
const { inferVerificationStatus } = require("./verification-evidence.cjs");

const REPORT_RELATIVE_PATH = ".ai/analysis/ai.report.json";
const VERIFY_STAGING_RELATIVE_PATH = ".ai/analysis/verify-staging.json";

function nowIso() {
  return new Date().toISOString();
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

function normalizeStatus(value, fallback = "n/a") {
  const normalized = String(value || fallback).trim().toLowerCase();
  if (["pass", "fail", "skip", "n/a"].includes(normalized)) {
    return normalized;
  }
  if (normalized === "ok" || normalized === "passed") {
    return "pass";
  }
  if (normalized === "failed") {
    return "fail";
  }
  return fallback;
}

function buildDefaultEntry(phaseName, message) {
  return {
    status: "n/a",
    command: "n/a",
    exit_code: 0,
    key_signal: message || `No ${phaseName} evidence has been recorded by runtime.`,
    source: "runtime-default",
    recorded_at: null,
  };
}

function normalizeVerificationEntry(stagingEntry, existingEntry, phaseName) {
  if (stagingEntry && typeof stagingEntry === "object") {
    return {
      status: normalizeStatus(stagingEntry.status),
      command: String(stagingEntry.command || "n/a"),
      exit_code: Number.isFinite(Number(stagingEntry.exit_code)) ? Number(stagingEntry.exit_code) : 0,
      key_signal: String(stagingEntry.key_signal || "runtime evidence recorded"),
      source: "verify-staging",
      recorded_at: String(stagingEntry.ts || "") || null,
    };
  }
  if (existingEntry && typeof existingEntry === "object" && !Array.isArray(existingEntry)) {
    return {
      status: inferVerificationStatus(existingEntry),
      command: String(existingEntry.command || "n/a"),
      exit_code: Number.isFinite(Number(existingEntry.exit_code)) ? Number(existingEntry.exit_code) : 0,
      key_signal: String(existingEntry.key_signal || existingEntry.signal || "existing report evidence"),
      source: String(existingEntry.source || "existing-report"),
      recorded_at: String(existingEntry.recorded_at || existingEntry.ts || "") || null,
    };
  }
  if (typeof existingEntry === "string") {
    return {
      ...buildDefaultEntry(phaseName, "Legacy report state preserved."),
      status: normalizeStatus(existingEntry),
      source: "legacy-report",
    };
  }
  return buildDefaultEntry(phaseName);
}

function normalizeTestEntries(stagingEntry, existingVerification) {
  if (stagingEntry && typeof stagingEntry === "object") {
    return [
      {
        status: normalizeStatus(stagingEntry.status),
        command: String(stagingEntry.command || "n/a"),
        exit_code: Number.isFinite(Number(stagingEntry.exit_code)) ? Number(stagingEntry.exit_code) : 0,
        key_signal: String(stagingEntry.key_signal || "runtime evidence recorded"),
        source: "verify-staging",
        recorded_at: String(stagingEntry.ts || "") || null,
      },
    ];
  }
  if (Array.isArray(existingVerification?.tests) && existingVerification.tests.length > 0) {
    return existingVerification.tests.map((entry) => ({
      status: inferVerificationStatus(entry),
      command: String(entry?.command || "n/a"),
      exit_code: Number.isFinite(Number(entry?.exit_code)) ? Number(entry.exit_code) : 0,
      key_signal: String(entry?.key_signal || entry?.signal || "existing report evidence"),
      source: String(entry?.source || "existing-report"),
      recorded_at: String(entry?.recorded_at || entry?.ts || "") || null,
    }));
  }
  if (typeof existingVerification?.test === "string") {
    return [
      {
        status: normalizeStatus(existingVerification.test),
        command: "n/a",
        exit_code: 0,
        key_signal: "Legacy report test state preserved.",
        source: "legacy-report",
        recorded_at: null,
      },
    ];
  }
  return [];
}

function statusFromTests(entries) {
  const tests = Array.isArray(entries) ? entries : [];
  if (tests.some((entry) => normalizeStatus(entry?.status) === "fail")) {
    return "fail";
  }
  if (tests.some((entry) => normalizeStatus(entry?.status) === "pass")) {
    return "pass";
  }
  if (tests.some((entry) => normalizeStatus(entry?.status) === "skip")) {
    return "skip";
  }
  return "n/a";
}

function summarizeNodes(taskGraph) {
  const nodes = Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : [];
  return {
    total_nodes: nodes.length,
    completed_nodes: nodes.filter((node) => node.status === "completed").length,
    failed_nodes: nodes.filter((node) => node.status === "failed").length,
  };
}

function buildApplicableChecks(verification) {
  let count = 0;
  for (const key of ["build", "typecheck", "lint", "security"]) {
    if (normalizeStatus(verification?.[key]?.status) !== "n/a") {
      count += 1;
    }
  }
  if (statusFromTests(verification?.tests) !== "n/a") {
    count += 1;
  }
  return count;
}

function findLatestVerificationEvent(rootDir) {
  const events = readJournalEvents(rootDir).filter((event) => event.event === "verification_recorded");
  return events.length > 0 ? events[events.length - 1] : null;
}

function determineOverall(session, verification) {
  const blockingStates = [
    normalizeStatus(verification?.build?.status),
    normalizeStatus(verification?.typecheck?.status),
    normalizeStatus(verification?.lint?.status),
    statusFromTests(verification?.tests),
    normalizeStatus(verification?.security?.status),
    normalizeStatus(verification?.coverage?.status),
  ];
  if (blockingStates.includes("fail")) {
    return "NOT_READY";
  }
  if (session?.approvals?.pending || Number(session?.approvals?.pending_count || 0) > 0) {
    return "NOT_READY";
  }
  if (session?.recovery?.restore_pending) {
    return "NOT_READY";
  }
  if (!["review", "completed"].includes(String(session?.phase?.status || ""))) {
    return "NOT_READY";
  }
  if (blockingStates.includes("pass")) {
    return "READY";
  }
  return "NOT_READY";
}

function buildVerificationBlock(rootDir, existingReport, staging) {
  const verification = clone(existingReport?.verification || {});
  const phases = staging?.phases || {};
  const latestEvent = findLatestVerificationEvent(rootDir);
  const coverageEvidence = loadCoverageEvidence(rootDir, { autoDiscover: true });

  const buildEntry = normalizeVerificationEntry(phases.build, verification.build, "build");
  const typecheckEntry = normalizeVerificationEntry(phases.typecheck, verification.typecheck || verification.compile, "typecheck");
  const lintEntry = normalizeVerificationEntry(phases.lint, verification.lint, "lint");
  const testEntries = normalizeTestEntries(phases.test, verification);
  const securityEntry = normalizeVerificationEntry(phases.security, verification.security, "security");

  return {
    build: buildEntry,
    typecheck: typecheckEntry,
    lint: lintEntry,
    tests: testEntries,
    security: securityEntry,
    coverage:
      coverageEvidence
        ? {
            status: String(coverageEvidence.status || (coverageEvidence.pass === true ? "pass" : "fail")),
            actual: coverageEvidence.actual,
            required: coverageEvidence.required,
            coverage_mode: coverageEvidence.coverage_mode,
            coverage_profile: coverageEvidence.coverage_profile,
            actual_basis: coverageEvidence.actual_basis,
            globalRegressed: coverageEvidence.globalRegressed,
            touchedRegionRegressed: coverageEvidence.touchedRegionRegressed,
            characterizationAdded: coverageEvidence.characterizationAdded,
            source_ref: coverageEvidence.source_ref,
            source_format: coverageEvidence.source_format,
            reasons: clone(coverageEvidence.reasons || []),
            recorded_at: coverageEvidence.recorded_at || null,
          }
        : verification.coverage && typeof verification.coverage === "object"
          ? clone(verification.coverage)
          : {
              status: "n/a",
              actual: "n/a",
              required: "n/a",
              rationale: "Coverage evidence has not been normalized by runtime yet.",
            },
    diff_review:
      verification.diff_review && typeof verification.diff_review === "object"
        ? clone(verification.diff_review)
        : {
            status: "n/a",
            scope: [],
            key_signal: latestEvent
              ? `Latest verification evidence captured at ${latestEvent.ts}.`
              : "Runtime report builder does not infer diff-review evidence by default.",
          },
    compile: normalizeStatus(typecheckEntry.status),
    test: statusFromTests(testEntries),
  };
}

function buildReport(rootDir) {
  const rootPath = path.resolve(rootDir);
  const reportPath = path.join(rootPath, REPORT_RELATIVE_PATH);
  const stagingPath = path.join(rootPath, VERIFY_STAGING_RELATIVE_PATH);
  const session = readSession(rootPath);
  const taskGraph = readTaskGraph(rootPath);
  const existingReport = readJsonIfExists(reportPath) || {};
  const staging = readJsonIfExists(stagingPath) || {};
  const timestamp = nowIso();
  const verification = buildVerificationBlock(rootPath, existingReport, staging);
  const nodeSummary = summarizeNodes(taskGraph);

  return {
    report_name: "ai.report",
    report_version: "v4",
    generated_at: String(existingReport.generated_at || timestamp),
    updated_at: timestamp,
    summary: {
      task: String(session?.task?.title || existingReport?.summary?.task || "Runtime verification report"),
      phase: String(session?.phase?.current || "unknown"),
      phase_status: String(session?.phase?.status || "unknown"),
      active_node: String(session?.node?.active_id || "none"),
      ...nodeSummary,
      applicable_checks: buildApplicableChecks(verification),
    },
    verification: {
      build: verification.build,
      typecheck: verification.typecheck,
      lint: verification.lint,
      tests: verification.tests,
      security: verification.security,
      coverage: verification.coverage,
      diff_review: verification.diff_review,
      compile: verification.compile,
      test: verification.test,
    },
    intent_delta:
      existingReport.intent_delta && typeof existingReport.intent_delta === "object"
        ? clone(existingReport.intent_delta)
        : {
            status: "n/a",
            expected: "Runtime-native verification report refreshed from durable state.",
            observed: "Verification evidence was collected from runtime assets instead of chat memory.",
          },
    security_findings: Array.isArray(existingReport.security_findings) ? clone(existingReport.security_findings) : [],
    scope_warnings: Array.isArray(existingReport.scope_warnings) ? clone(existingReport.scope_warnings) : [],
    overall: determineOverall(session, verification),
    report_ref: REPORT_RELATIVE_PATH,
    staging_ref: fs.existsSync(stagingPath) ? VERIFY_STAGING_RELATIVE_PATH : null,
  };
}

function writeReport(rootDir) {
  const result = buildReport(rootDir);
  writeJson(path.join(path.resolve(rootDir), REPORT_RELATIVE_PATH), {
    report_name: result.report_name,
    report_version: result.report_version,
    generated_at: result.generated_at,
    updated_at: result.updated_at,
    summary: result.summary,
    verification: result.verification,
    intent_delta: result.intent_delta,
    security_findings: result.security_findings,
    scope_warnings: result.scope_warnings,
    overall: result.overall,
  });
  return result;
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    write: false,
    json: false,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
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
    const result = parsed.write ? writeReport(parsed.rootDir) : buildReport(parsed.rootDir);
    if (parsed.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }
    console.log(`[report-builder] overall=${result.overall} report=${result.report_ref}`);
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  buildReport,
  parseArgs,
  writeReport,
};

if (require.main === module) {
  main();
}
