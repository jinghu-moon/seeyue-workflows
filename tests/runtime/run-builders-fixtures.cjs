#!/usr/bin/env node
"use strict";

// P1-N3: Request Builders fixtures
// Tests for buildApprovalRequest, buildRestoreRequest, buildQuestionRequest, buildInputRequest

const fs = require("node:fs");
const os = require("node:os");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── RED: require builders module ────────────────────────────────────────────
const {
  buildApprovalRequest,
  buildRestoreRequest,
  buildQuestionRequest,
  buildInputRequest,
} = require("../../scripts/runtime/interaction-builders.cjs");

const IX_ID_PATTERN = /^ix-\d{8}-\d{3,}$/;

const cases = {};

// ─── CASE: buildApprovalRequest schema ───────────────────────────────────────
cases["builders-approval-schema"] = function () {
  const req = buildApprovalRequest({
    subject: "Destructive file write requires approval",
    detail: "Will overwrite config.yaml",
    risk_level: "high",
    originating_request_id: "req-bash-001",
    options: [
      { id: "approve", label: "批准", recommended: true },
      { id: "reject", label: "拒绝", recommended: false },
    ],
  });

  assert(req.schema === 1, "schema must be 1");
  assert(typeof req.interaction_id === "string", "interaction_id must be string");
  assert(IX_ID_PATTERN.test(req.interaction_id), `interaction_id must match pattern, got ${req.interaction_id}`);
  assert(req.kind === "approval_request", "kind must be approval_request");
  assert(req.status === "pending", "status must be pending");
  assert(typeof req.title === "string" && req.title.length > 0, "title must be non-empty");
  assert(typeof req.message === "string", "message must be string");
  assert(req.selection_mode === "boolean", "selection_mode must be boolean for approval");
  assert(Array.isArray(req.options), "options must be array");
  assert(req.options.length >= 2, "approval must have at least 2 options");
  assert(req.comment_mode === "disabled" || req.comment_mode === "optional", "comment_mode must be disabled or optional");
  assert(req.presentation !== null && typeof req.presentation === "object", "presentation must be object");
  assert(req.presentation.mode === "text_menu", "default presentation mode must be text_menu");
  assert(req.presentation.color_profile === "auto", "default color_profile must be auto");
  assert(req.presentation.theme === "auto", "default theme must be auto");
  assert(typeof req.originating_request_id === "string", "originating_request_id must be string");
  assert(typeof req.created_at === "string", "created_at must be string");
};

// ─── CASE: buildRestoreRequest schema ────────────────────────────────────────
cases["builders-restore-schema"] = function () {
  const req = buildRestoreRequest({
    restore_reason: "journal_orphan_detected",
    checkpoint_id: "node-1234-abcd",
    originating_request_id: "req-stop-001",
  });

  assert(req.schema === 1, "schema must be 1");
  assert(IX_ID_PATTERN.test(req.interaction_id), `interaction_id pattern, got ${req.interaction_id}`);
  assert(req.kind === "restore_request", "kind must be restore_request");
  assert(req.status === "pending", "status must be pending");
  assert(typeof req.title === "string" && req.title.length > 0, "title must be non-empty");
  assert(req.selection_mode === "single_select" || req.selection_mode === "boolean", "restore must have single_select or boolean");
  assert(Array.isArray(req.options) && req.options.length > 0, "restore must have options");
  assert(typeof req.created_at === "string", "created_at must be string");
};

// ─── CASE: buildQuestionRequest schema ───────────────────────────────────────
cases["builders-question-schema"] = function () {
  const req = buildQuestionRequest({
    question: "Which branch should be used?",
    options: [
      { id: "main", label: "main" },
      { id: "dev", label: "dev" },
    ],
    originating_request_id: "req-q-001",
  });

  assert(req.schema === 1, "schema must be 1");
  assert(IX_ID_PATTERN.test(req.interaction_id), `interaction_id pattern, got ${req.interaction_id}`);
  assert(req.kind === "question_request", "kind must be question_request");
  assert(req.status === "pending", "status must be pending");
  assert(Array.isArray(req.options), "options must be array");
  assert(typeof req.created_at === "string", "created_at must be string");
};

// ─── CASE: buildInputRequest schema ──────────────────────────────────────────
cases["builders-input-schema"] = function () {
  const req = buildInputRequest({
    prompt: "Enter the target file path:",
    kind: "file_path",
    originating_request_id: "req-input-001",
  });

  assert(req.schema === 1, "schema must be 1");
  assert(IX_ID_PATTERN.test(req.interaction_id), `interaction_id pattern, got ${req.interaction_id}`);
  assert(req.kind === "input_request", "kind must be input_request");
  assert(req.status === "pending", "status must be pending");
  assert(req.selection_mode === "text" || req.selection_mode === "path" || req.selection_mode === "secret",
    `selection_mode must be text/path/secret for input, got ${req.selection_mode}`);
  assert(typeof req.created_at === "string", "created_at must be string");
};

// ─── CASE: unique interaction_ids ────────────────────────────────────────────
cases["builders-unique-ids"] = function () {
  const ids = new Set();
  for (let i = 0; i < 10; i++) {
    const req = buildApprovalRequest({
      subject: `Approval ${i}`,
      originating_request_id: `req-${i}`,
      options: [
        { id: "approve", label: "批准", recommended: true },
        { id: "reject", label: "拒绝", recommended: false },
      ],
    });
    assert(IX_ID_PATTERN.test(req.interaction_id), `id ${req.interaction_id} must match pattern`);
    ids.add(req.interaction_id);
  }
  // IDs generated in the same millisecond may collide — but the implementation
  // should use a counter suffix to keep them unique
  assert(ids.size === 10, `all 10 ids must be unique, got ${ids.size}`);
};

// ─── Runner ──────────────────────────────────────────────────────────────────
function parseArgs(argv) {
  const parsed = { caseName: null };
  for (let index = 0; index < argv.length; index += 1) {
    if (argv[index] === "--case") {
      index += 1;
      parsed.caseName = argv[index];
      continue;
    }
    throw new Error(`Unknown argument: ${argv[index]}`);
  }
  return parsed;
}

function main() {
  const parsed = parseArgs(process.argv.slice(2));
  const selected = parsed.caseName ? [[parsed.caseName, cases[parsed.caseName]]] : Object.entries(cases);
  if (selected.some(([, run]) => typeof run !== "function")) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }
  let failed = false;
  for (const [caseName, run] of selected) {
    try {
      run();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
    }
  }
  if (failed) {
    process.exit(1);
  }
  console.log("BUILDERS_FIXTURES_PASS");
}

main();
