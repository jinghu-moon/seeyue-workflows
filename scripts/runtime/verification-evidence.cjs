"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { loadCoverageEvidence } = require("./coverage-adapter.cjs");
const { readSession, readTaskGraph } = require("./store.cjs");
const { getActiveNode } = require("./runtime-state.cjs");

const REPORT_RELATIVE_PATH = ".ai/analysis/ai.report.json";
const VERIFY_STAGING_RELATIVE_PATH = ".ai/analysis/verify-staging.json";
const PHASE_ORDER = ["build", "typecheck", "lint", "test", "security", "verify"];

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

function normalizeStatus(value, fallback = "n/a") {
  const normalized = String(value || fallback).trim().toLowerCase();
  if (["pass", "fail", "skip", "n/a"].includes(normalized)) {
    return normalized;
  }
  if (["ok", "passed", "ready"].includes(normalized)) {
    return "pass";
  }
  if (["failed", "error"].includes(normalized)) {
    return "fail";
  }
  return fallback;
}

function inferVerificationStatus(entry, fallback = "n/a") {
  if (!entry || typeof entry !== "object") {
    return fallback;
  }
  const explicit = normalizeStatus(entry.status, fallback);
  if (explicit !== "n/a") {
    return explicit;
  }
  const command = String(entry.command || "").trim();
  const exitCode = Number(entry.exit_code);
  if (command && command !== "n/a" && Number.isFinite(exitCode)) {
    return exitCode === 0 ? "pass" : "fail";
  }
  return fallback;
}

function buildPhaseEntryFromObject(entry) {
  if (!entry || typeof entry !== "object" || Array.isArray(entry)) {
    return null;
  }
  return {
    command: String(entry.command || "n/a"),
    exit_code: Number.isFinite(Number(entry.exit_code)) ? Number(entry.exit_code) : 0,
    status: inferVerificationStatus(entry),
    key_signal: String(entry.key_signal || entry.signal || "runtime evidence recorded"),
    ts: String(entry.recorded_at || entry.ts || "") || null,
    source: String(entry.source || "report-fallback"),
  };
}

function buildTestPhaseEntryFromReport(report) {
  const tests = Array.isArray(report?.verification?.tests) ? report.verification.tests : [];
  if (tests.length === 0) {
    return null;
  }
  const normalized = tests.map((entry) => buildPhaseEntryFromObject(entry)).filter(Boolean);
  if (normalized.length === 0) {
    return null;
  }
  const statuses = normalized.map((entry) => entry.status);
  const aggregateStatus = statuses.includes("fail") ? "fail" : statuses.includes("pass") ? "pass" : statuses.includes("skip") ? "skip" : "n/a";
  return {
    command: normalized.map((entry) => entry.command).filter((value) => value && value !== "n/a").join(" && ") || "n/a",
    exit_code: aggregateStatus === "fail" ? 1 : 0,
    status: aggregateStatus,
    key_signal: normalized[0].key_signal,
    ts: normalized[0].ts,
    source: normalized[0].source,
  };
}

function buildReportPhaseEntries(report) {
  const verification = report?.verification || {};
  return {
    build: buildPhaseEntryFromObject(verification.build),
    typecheck: buildPhaseEntryFromObject(verification.typecheck),
    lint: buildPhaseEntryFromObject(verification.lint),
    test: buildTestPhaseEntryFromReport(report),
    security: buildPhaseEntryFromObject(verification.security),
    verify: null,
  };
}

function summarizePhaseEntries(phaseEntries) {
  const observedPhases = PHASE_ORDER.filter((phase) => phaseEntries[phase] && phaseEntries[phase].status !== "n/a");
  const passingPhases = observedPhases.filter((phase) => phaseEntries[phase].status === "pass");
  const failingPhases = observedPhases.filter((phase) => phaseEntries[phase].status === "fail");
  const verifyPassed = failingPhases.length === 0 && passingPhases.length > 0;
  return {
    observedPhases,
    passingPhases,
    failingPhases,
    verifyPassed,
  };
}

function loadVerificationEvidence(rootDir) {
  const rootPath = path.resolve(rootDir);
  const report = readJsonIfExists(path.join(rootPath, REPORT_RELATIVE_PATH)) || {};
  const staging = readJsonIfExists(path.join(rootPath, VERIFY_STAGING_RELATIVE_PATH)) || {};
  const reportPhases = buildReportPhaseEntries(report);
  const stagingPhases = staging?.phases && typeof staging.phases === "object" ? staging.phases : {};
  const phaseEntries = {};

  for (const phase of PHASE_ORDER) {
    const staged = buildPhaseEntryFromObject(stagingPhases[phase]);
    phaseEntries[phase] = staged || reportPhases[phase] || null;
  }

  const summary = summarizePhaseEntries(phaseEntries);
  const coverageEvidence = loadCoverageEvidence(rootPath, { autoDiscover: true });

  return {
    report,
    staging,
    phase_entries: phaseEntries,
    updated_at: staging?.updated_at || report?.updated_at || null,
    source: summary.observedPhases.some((phase) => buildPhaseEntryFromObject(stagingPhases[phase])) ? "verify-staging" : report?.report_name ? "report-fallback" : "none",
    coverage_evidence: coverageEvidence,
    ...summary,
  };
}

function buildVerifyActionContext(rootDir, actionContext = {}) {
  const merged = clone(actionContext) || {};
  const verification = loadVerificationEvidence(rootDir);
  const testPhase = verification.phase_entries.test;

  if (merged.verifyEvidence === undefined) {
    merged.verifyEvidence = {
      executed: verification.observedPhases.length > 0,
      passed: verification.verifyPassed,
      source: verification.source,
      observedPhases: clone(verification.observedPhases),
      failingPhases: clone(verification.failingPhases),
      passingPhases: clone(verification.passingPhases),
      recorded_at: verification.updated_at,
    };
  }

  if (merged.verificationPassed === undefined) {
    merged.verificationPassed = verification.verifyPassed;
  }

  if (merged.greenEvidence === undefined && testPhase) {
    merged.greenEvidence = {
      executed: true,
      passed: testPhase.status === "pass",
      newBlockerIntroduced: false,
      source: verification.source,
      recorded_at: testPhase.ts,
    };
  }

  if (merged.coverageEvidence === undefined && verification.coverage_evidence) {
    merged.coverageEvidence = verification.coverage_evidence;
  }

  return merged;
}

function buildReviewActionContext(rootDir, actionContext = {}, reviewContext = {}) {
  const merged = buildVerifyActionContext(rootDir, actionContext);
  const session = readSession(rootDir);
  const taskGraph = readTaskGraph(rootDir);
  const activeNode = getActiveNode(session || {}, taskGraph || {});

  if (merged.reviewEvidence === undefined) {
    merged.reviewEvidence = {
      present: true,
      fresh: true,
      reviewer: reviewContext.persona || session?.node?.owner_persona || null,
      decision: reviewContext.decision || null,
      recorded_at: reviewContext.recordedAt || new Date().toISOString(),
      source: reviewContext.source || "review-resolution",
    };
  }

  if (
    merged.greenEvidence === undefined &&
    activeNode?.tdd_required === false &&
    merged.verifyEvidence?.passed === true
  ) {
    merged.greenEvidence = {
      executed: true,
      passed: true,
      newBlockerIntroduced: false,
      source: reviewContext.source || "review-resolution",
      recorded_at: reviewContext.recordedAt || merged.verifyEvidence?.recorded_at || null,
    };
  }

  return merged;
}

module.exports = {
  REPORT_RELATIVE_PATH,
  VERIFY_STAGING_RELATIVE_PATH,
  buildReviewActionContext,
  buildVerifyActionContext,
  inferVerificationStatus,
  loadVerificationEvidence,
  normalizeStatus,
  readJsonIfExists,
};
