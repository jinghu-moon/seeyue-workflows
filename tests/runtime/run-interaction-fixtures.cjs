#!/usr/bin/env node
"use strict";

// P1-N1: Interaction Store fixtures
// RED: this will fail until interaction-store.cjs is implemented

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-interaction-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── RED: require store module (will fail if not yet created) ───────────────
const {
  ensureInteractionLayout,
  writeRequest,
  readRequest,
  writeResponse,
  readResponse,
  getActive,
  setActive,
  clearActive,
  archiveInteraction,
  listPending,
} = require("../../scripts/runtime/interaction-store.cjs");

const cases = {};

// ─── CASE: layout creation ───────────────────────────────────────────────────
cases["store-layout"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const base = path.join(root, ".ai", "workflow", "interactions");
  assert(fs.existsSync(path.join(base, "requests")), "requests dir must exist");
  assert(fs.existsSync(path.join(base, "responses")), "responses dir must exist");
  assert(fs.existsSync(path.join(base, "archive")), "archive dir must exist");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: write and read request ───────────────────────────────────────────
cases["store-write-read-request"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const req = {
    schema: 1,
    interaction_id: "ix-20260318-001",
    kind: "approval_request",
    status: "pending",
    title: "Test Approval",
    message: "Do you approve?",
    selection_mode: "boolean",
    options: [],
    comment_mode: "disabled",
    presentation: { mode: "text_menu", color_profile: "auto", theme: "auto" },
    originating_request_id: "req-001",
    created_at: new Date().toISOString(),
  };
  writeRequest(root, req);
  const loaded = readRequest(root, "ix-20260318-001");
  assert(loaded !== null, "loaded request must not be null");
  assert(loaded.interaction_id === "ix-20260318-001", "interaction_id must match");
  assert(loaded.kind === "approval_request", "kind must match");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: write and read response ──────────────────────────────────────────
cases["store-write-read-response"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const resp = {
    schema: 1,
    interaction_id: "ix-20260318-001",
    status: "answered",
    selected_option_ids: ["approve"],
    comment: null,
    presenter_mode: "text_menu",
    answered_at: new Date().toISOString(),
  };
  writeResponse(root, resp);
  const loaded = readResponse(root, "ix-20260318-001");
  assert(loaded !== null, "loaded response must not be null");
  assert(loaded.interaction_id === "ix-20260318-001", "interaction_id must match");
  assert(loaded.status === "answered", "status must be answered");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: active.json set/get/clear ────────────────────────────────────────
cases["store-active"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  assert(getActive(root) === null, "active must be null initially");
  const activeObj = {
    active_id: "ix-20260318-001",
    pending_count: 1,
    blocking_kind: "approval",
    blocking_reason: "destructive_write_requires_approval",
    created_at: new Date().toISOString(),
  };
  setActive(root, activeObj);
  const loaded = getActive(root);
  assert(loaded !== null, "active must not be null after set");
  assert(loaded.active_id === "ix-20260318-001", "active_id must match");
  assert(loaded.blocking_kind === "approval", "blocking_kind must match");
  clearActive(root);
  assert(getActive(root) === null, "active must be null after clear");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: listPending ───────────────────────────────────────────────────────
cases["store-list-pending"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const req1 = {
    schema: 1,
    interaction_id: "ix-20260318-001",
    kind: "approval_request",
    status: "pending",
    title: "Req 1",
    message: "msg",
    selection_mode: "boolean",
    options: [],
    comment_mode: "disabled",
    presentation: { mode: "text_menu", color_profile: "auto", theme: "auto" },
    originating_request_id: "req-001",
    created_at: new Date().toISOString(),
  };
  const req2 = {
    ...req1,
    interaction_id: "ix-20260318-002",
    status: "pending",
    originating_request_id: "req-002",
  };
  writeRequest(root, req1);
  writeRequest(root, req2);
  const pending = listPending(root);
  assert(pending.length === 2, `expected 2 pending, got ${pending.length}`);
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: archiveInteraction ────────────────────────────────────────────────
cases["store-archive"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const req = {
    schema: 1,
    interaction_id: "ix-20260318-001",
    kind: "approval_request",
    status: "pending",
    title: "Archive Test",
    message: "msg",
    selection_mode: "boolean",
    options: [],
    comment_mode: "disabled",
    presentation: { mode: "text_menu", color_profile: "auto", theme: "auto" },
    originating_request_id: "req-001",
    created_at: new Date().toISOString(),
  };
  const resp = {
    schema: 1,
    interaction_id: "ix-20260318-001",
    status: "answered",
    selected_option_ids: ["approve"],
    comment: null,
    presenter_mode: "text_menu",
    answered_at: new Date().toISOString(),
  };
  writeRequest(root, req);
  writeResponse(root, resp);
  archiveInteraction(root, "ix-20260318-001");

  // After archive: request file should not exist in requests/ dir
  const base = path.join(root, ".ai", "workflow", "interactions");
  const reqFile = path.join(base, "requests", "ix-20260318-001.json");
  assert(!fs.existsSync(reqFile), "request file must be removed from requests/ after archive");

  // Archive file must exist
  const archiveFile = path.join(base, "archive", "ix-20260318-001.json");
  assert(fs.existsSync(archiveFile), "archive file must exist after archive");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: readRequest returns null for missing ───────────────────────────
cases["store-read-missing"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const result = readRequest(root, "ix-99999999-999");
  assert(result === null, "readRequest must return null for missing id");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── P1-N8: End-to-End Local Loop Fixtures ─────────────────────────────────

const { dispatchInteraction } = require("../../scripts/runtime/interaction-dispatcher.cjs");
const { buildApprovalRequest } = require("../../scripts/runtime/interaction-builders.cjs");
const { setInteractionBlock, clearInteractionBlock, getInteractionBlock } = require("../../scripts/runtime/interaction-projection.cjs");
const { appendInteractionEvent, INTERACTION_EVENTS } = require("../../scripts/runtime/interaction-journal.cjs");
const { writeSession, writeTaskGraph, writeSprintStatus } = require("../../scripts/runtime/store.cjs");

function makeBaseSession(runId) {
  return {
    schema: 4,
    run_id: runId || "wf-20260318-e2e-001",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-p1", title: "E2E test", mode: "feature" },
    phase: { current: "P1", status: "in_progress" },
    node: { active_id: "P1-N8", state: "red_pending", owner_persona: "author" },
    loop_budget: { max_nodes: 5, max_failures: 2, max_pending_approvals: 2, consumed_nodes: 0, consumed_failures: 0 },
    context_budget: { strategy: "hybrid", capsule_refresh_threshold: 4, summary_required_after_turns: 8 },
    workspace: { root: "D:/repo/demo", sandbox_mode: "workspace_write" },
    approvals: { pending: false, pending_count: 0, last_grant_scope: "none", last_approval_mode: "none", active_request: null, grants: [] },
    recovery: { last_checkpoint_id: null, restore_pending: false, restore_reason: null },
    timestamps: { created_at: "2026-03-18T12:00:00Z", updated_at: "2026-03-18T12:00:00Z" },
  };
}

function makeBaseTaskGraph() {
  return {
    schema: 4,
    phases: [{ id: "P1", title: "P1", status: "in_progress" }],
    nodes: [{ id: "P1-N8", phase_id: "P1", title: "N8", status: "in_progress", capability: "code_edit", owner_persona: "author", depends_on: [] }],
  };
}

function makeBaseSprintStatus() {
  return { schema: 4, sprint: { id: "sp-1", title: "Sprint 1", status: "in_progress" } };
}

// Build a node shim eval string that writes a response JSON to the given path and exits with exitCode
function makeShimEval(responseFilePath, responseObj, exitCode) {
  const safeResponsePath = responseFilePath.replace(/\\/g, "/");
  const responseJson = JSON.stringify(responseObj);
  return [
    `const fs = require('node:fs');`,
    `fs.writeFileSync(${JSON.stringify(safeResponsePath)}, ${JSON.stringify(responseJson)}, 'utf8');`,
    `process.exit(${exitCode});`,
  ].join(" ");
}

// ─── CASE: local-loop-incomplete (RED gate) ───────────────────────────────────
// This case verifies that all required P1-N8 modules can be required.
// It fails until all dependencies are importable — now it should pass.
cases["local-loop-incomplete"] = function () {
  assert(typeof dispatchInteraction === "function", "dispatchInteraction must be exported");
  assert(typeof buildApprovalRequest === "function", "buildApprovalRequest must be exported");
  assert(typeof setInteractionBlock === "function", "setInteractionBlock must be exported");
  assert(typeof appendInteractionEvent === "function", "appendInteractionEvent must be exported");
  assert(typeof INTERACTION_EVENTS === "object", "INTERACTION_EVENTS must be exported");
};

// ─── CASE: local-loop-happy-path ─────────────────────────────────────────────
// Full round-trip: build request → write to store → simulate presenter writing
// completed response → dispatch reads it → verify response status=completed
cases["local-loop-happy-path"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const session = makeBaseSession();
  writeSession(root, session);
  writeTaskGraph(root, makeBaseTaskGraph());
  writeSprintStatus(root, makeBaseSprintStatus());

  const req = buildApprovalRequest({
    subject: "Approve destructive write",
    message: "Allow deletion of build artifacts?",
    options: [
      { id: "approve", label: "Approve", recommended: true, destructive: false, shortcut: "y" },
      { id: "deny",    label: "Deny",    recommended: false, destructive: false, shortcut: "n" },
    ],
    originatingRequestId: "req-happy-001",
  });
  writeRequest(root, req);

  // Simulate presenter: write response file directly, exit 0
  const interactionsBase = path.join(root, ".ai", "workflow", "interactions");
  const responseFilePath = path.join(interactionsBase, "responses", `${req.interaction_id}.json`);
  const responseObj = {
    schema: 1,
    interaction_id: req.interaction_id,
    status: "completed",
    selected_option_ids: ["approve"],
    comment: null,
    presenter_mode_used: "plain",
    responded_at: new Date().toISOString(),
  };

  const shimEval = makeShimEval(responseFilePath, responseObj, 0);
  const result = dispatchInteraction(root, req.interaction_id, {
    binaryPath: process.execPath,
    extraArgs: ["--eval", shimEval],
    mode: "plain",
  });

  assert(result.exitCode === 0, `expected exitCode 0, got ${result.exitCode}`);
  assert(result.error === null, `expected no error, got ${result.error}`);
  assert(result.response !== null, "response must not be null");
  assert(result.response.status === "completed", `expected status completed, got ${result.response.status}`);
  assert(result.response.selected_option_ids[0] === "approve", "selected option must be approve");

  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: local-loop-cancel-path ────────────────────────────────────────────
// Presenter exits with code 2 (user cancelled) and writes cancelled response.
cases["local-loop-cancel-path"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const session = makeBaseSession("wf-20260318-e2e-002");
  writeSession(root, session);
  writeTaskGraph(root, makeBaseTaskGraph());
  writeSprintStatus(root, makeBaseSprintStatus());

  const req = buildApprovalRequest({
    subject: "Approve",
    message: "Approve?",
    options: [
      { id: "approve", label: "Approve", recommended: true,  destructive: false, shortcut: "y" },
      { id: "deny",    label: "Deny",    recommended: false, destructive: false, shortcut: "n" },
    ],
    originatingRequestId: "req-cancel-001",
  });
  writeRequest(root, req);

  const interactionsBase = path.join(root, ".ai", "workflow", "interactions");
  const responseFilePath = path.join(interactionsBase, "responses", `${req.interaction_id}.json`);
  const responseObj = {
    schema: 1,
    interaction_id: req.interaction_id,
    status: "cancelled",
    selected_option_ids: [],
    comment: null,
    presenter_mode_used: "plain",
    responded_at: new Date().toISOString(),
  };

  const shimEval = makeShimEval(responseFilePath, responseObj, 2);
  const result = dispatchInteraction(root, req.interaction_id, {
    binaryPath: process.execPath,
    extraArgs: ["--eval", shimEval],
    mode: "plain",
  });

  assert(result.exitCode === 2, `expected exitCode 2, got ${result.exitCode}`);
  assert(result.response !== null, "response must not be null for cancel");
  assert(result.response.status === "cancelled", `expected status cancelled, got ${result.response.status}`);

  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: local-loop-non-tty-fallback ───────────────────────────────────────
// Verifies that plain mode is selected when not a TTY and round-trip completes.
cases["local-loop-non-tty-fallback"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const session = makeBaseSession("wf-20260318-e2e-003");
  writeSession(root, session);
  writeTaskGraph(root, makeBaseTaskGraph());
  writeSprintStatus(root, makeBaseSprintStatus());

  const req = buildApprovalRequest({
    subject: "Non-TTY test",
    message: "Does plain fallback work?",
    options: [
      { id: "ok",   label: "OK",     recommended: true,  destructive: false, shortcut: "y" },
      { id: "skip", label: "Skip",   recommended: false, destructive: false, shortcut: "n" },
    ],
    originatingRequestId: "req-nontty-001",
  });
  writeRequest(root, req);

  const interactionsBase = path.join(root, ".ai", "workflow", "interactions");
  const responseFilePath = path.join(interactionsBase, "responses", `${req.interaction_id}.json`);
  const responseObj = {
    schema: 1,
    interaction_id: req.interaction_id,
    status: "completed",
    selected_option_ids: ["ok"],
    comment: null,
    presenter_mode_used: "plain",
    responded_at: new Date().toISOString(),
  };

  const shimEval = makeShimEval(responseFilePath, responseObj, 0);
  // Force mode to plain (non-TTY simulation)
  const result = dispatchInteraction(root, req.interaction_id, {
    binaryPath: process.execPath,
    extraArgs: ["--eval", shimEval],
    mode: "plain",
  });

  assert(result.exitCode === 0, `expected exitCode 0, got ${result.exitCode}`);
  assert(result.response.status === "completed", "status must be completed");
  assert(result.response.presenter_mode_used === "plain", "presenter_mode_used must be plain");

  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: local-loop-round-trip ─────────────────────────────────────────────
// Full integration: build → store → setInteractionBlock → dispatch → journal event
// → clearInteractionBlock → archive. Verifies all P1 modules work together.
cases["local-loop-round-trip"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const session = makeBaseSession("wf-20260318-e2e-004");
  writeSession(root, session);
  writeTaskGraph(root, makeBaseTaskGraph());
  writeSprintStatus(root, makeBaseSprintStatus());

  // 1. Build and write request
  const req = buildApprovalRequest({
    subject: "Round-trip test",
    message: "Full P1 loop?",
    options: [
      { id: "yes", label: "Yes", recommended: true,  destructive: false, shortcut: "y" },
      { id: "no",  label: "No",  recommended: false, destructive: false, shortcut: "n" },
    ],
    originatingRequestId: "req-roundtrip-001",
  });
  writeRequest(root, req);

  // 2. Set interaction block in session
  setInteractionBlock(root, session, {
    active_interaction_id: req.interaction_id,
    pending_count: 1,
    last_dispatched_at: null,
    blocking_kind: "approval",
    blocking_reason: "round_trip_test",
  });

  // 3. Emit interaction_created journal event
  appendInteractionEvent(root, {
    event: INTERACTION_EVENTS.interaction_created,
    runId: session.run_id,
    interactionId: req.interaction_id,
    blockingKind: "approval",
  });

  // 4. Dispatch (shim writes completed response)
  const interactionsBase = path.join(root, ".ai", "workflow", "interactions");
  const responseFilePath = path.join(interactionsBase, "responses", `${req.interaction_id}.json`);
  const responseObj = {
    schema: 1,
    interaction_id: req.interaction_id,
    status: "completed",
    selected_option_ids: ["yes"],
    comment: "Approved",
    presenter_mode_used: "plain",
    responded_at: new Date().toISOString(),
  };
  const shimEval = makeShimEval(responseFilePath, responseObj, 0);
  const result = dispatchInteraction(root, req.interaction_id, {
    binaryPath: process.execPath,
    extraArgs: ["--eval", shimEval],
    mode: "plain",
  });

  assert(result.exitCode === 0, `expected exitCode 0, got ${result.exitCode}`);
  assert(result.response.status === "completed", "response status must be completed");

  // 5. Emit interaction_answered journal event
  appendInteractionEvent(root, {
    event: INTERACTION_EVENTS.interaction_answered,
    runId: session.run_id,
    interactionId: req.interaction_id,
    blockingKind: "approval",
  });

  // 6. Clear interaction block
  const updatedSession = require("../../scripts/runtime/store.cjs").readSession(root);
  clearInteractionBlock(root, updatedSession);
  const clearedSession = require("../../scripts/runtime/store.cjs").readSession(root);
  const block = getInteractionBlock(clearedSession);
  assert(block === null, "interaction block must be cleared after resolution");

  // 7. Archive interaction
  archiveInteraction(root, req.interaction_id);
  const archiveFile = path.join(interactionsBase, "archive", `${req.interaction_id}.json`);
  assert(fs.existsSync(archiveFile), "archive file must exist after archiveInteraction");

  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: legacy-to-interaction-projection ─────────────────────────────────
// Verifies that the canonical interaction projection (active_interaction_id)
// survives a setInteractionBlock → getInteractionBlock round-trip.
// This is the GREEN gate for P2-N3: legacy tools must project into the
// unified interaction schema without field divergence.
cases["legacy-to-interaction-projection"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  const session = makeBaseSession("wf-20260318-proj-002");
  writeSession(root, session);
  writeTaskGraph(root, makeBaseTaskGraph());
  writeSprintStatus(root, makeBaseSprintStatus());

  // Simulate a legacy tool (approval/ask_user/input_request) projecting into
  // the canonical interaction block using active_interaction_id.
  setInteractionBlock(root, session, {
    active_interaction_id: "ix-20260318-legacy-001",
    pending_count: 1,
    last_dispatched_at: null,
    blocking_kind: "approval",
    blocking_reason: "legacy_tool_projection_test",
  });

  // Re-read and verify canonical field is intact.
  const loaded = require("../../scripts/runtime/store.cjs").readSession(root);
  const block = getInteractionBlock(loaded);
  assert(block !== null, "interaction block must exist after legacy projection");
  assert(
    block.active_interaction_id === "ix-20260318-legacy-001",
    `active_interaction_id must be canonical, got: ${block.active_interaction_id}`
  );
  assert(block.blocking_kind === "approval", "blocking_kind must be preserved");
  assert(block.pending_count === 1, "pending_count must be preserved");

  // Clear and verify projection resets to defaults (no stale active_interaction_id).
  clearInteractionBlock(root, loaded);
  const cleared = require("../../scripts/runtime/store.cjs").readSession(root);
  const clearedBlock = getInteractionBlock(cleared);
  assert(clearedBlock === null, "getInteractionBlock must return null after clear");

  fs.rmSync(root, { recursive: true, force: true });
};

// ─── P2-N5: Orchestration dispatch fixtures ──────────────────────────────────
//
// Verifies the elicitation-first orchestration in interaction-dispatch.cjs.
// Tests all three paths: elicitation, local_presenter (binary missing → fallback),
// and text_fallback.

const { orchestrateInteraction, selectStrategy } = require("../../scripts/runtime/interaction-dispatch.cjs");
const { buildApprovalRequest: buildApprovalReq2 } = require("../../scripts/runtime/interaction-builders.cjs");

cases["orchestration-strategy-elicitation"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);

  // Activate elicitation via capabilities.yaml
  const capDir = path.join(root, ".ai", "workflow");
  fs.mkdirSync(capDir, { recursive: true });
  fs.writeFileSync(path.join(capDir, "capabilities.yaml"), "elicitation: true\n");

  const { strategy } = selectStrategy(root, {});
  assert(strategy === "elicitation",
    `strategy must be elicitation when capabilities.yaml sets it, got: ${strategy}`);

  fs.rmSync(root, { recursive: true, force: true });
};

cases["orchestration-strategy-text-fallback"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);

  // No capabilities.yaml, no binary override → text_fallback or local_presenter
  const { strategy } = selectStrategy(root, {
    capabilitiesOverride: { supports_elicitation: false, supports_local_presenter: false },
  });
  assert(strategy === "text_fallback",
    `strategy must be text_fallback when no elicitation or presenter, got: ${strategy}`);

  fs.rmSync(root, { recursive: true, force: true });
};

cases["orchestration-elicitation-dispatch"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  writeSession(root, makeBaseSession("wf-p2n5-orch-001"));

  // Build and store a request
  const req = buildApprovalReq2({
    subject:      "Orchestration test",
    options: [
      { id: "approve", label: "Approve", recommended: true },
      { id: "reject",  label: "Reject",  recommended: false },
    ],
    originating_request_id: "ap-orch-001",
    risk_level: "medium",
  });
  writeRequest(root, req);

  // Dispatch via elicitation path
  const result = orchestrateInteraction(root, req.interaction_id, {
    capabilitiesOverride: { supports_elicitation: true, supports_local_presenter: false },
  });

  assert(result.strategy === "elicitation",
    `result.strategy must be elicitation, got: ${result.strategy}`);
  assert(result.exitCode === 0, `exitCode must be 0, got: ${result.exitCode}`);
  assert(result.error === null, `error must be null, got: ${result.error}`);
  assert(result.response !== null, "response must not be null");
  assert(result.response.status === "elicitation_pending",
    `response.status must be elicitation_pending, got: ${result.response.status}`);
  // Pre-resolution: elicitation is handled by MCP client — response store must stay empty
  const storedResp = readResponse(root, req.interaction_id);
  assert(storedResp === null,
    `response store must be empty for elicitation pre-resolution, got: ${JSON.stringify(storedResp)}`);

  fs.rmSync(root, { recursive: true, force: true });
};

cases["orchestration-text-fallback-dispatch"] = function () {
  const root = makeTempRoot();
  ensureInteractionLayout(root);
  writeSession(root, makeBaseSession("wf-p2n5-orch-002"));

  const req = buildApprovalReq2({
    subject:      "Fallback test",
    options: [
      { id: "approve", label: "Approve", recommended: true },
      { id: "reject",  label: "Reject",  recommended: false },
    ],
    originating_request_id: "ap-orch-002",
    risk_level: "low",
  });
  writeRequest(root, req);

  const result = orchestrateInteraction(root, req.interaction_id, {
    capabilitiesOverride: { supports_elicitation: false, supports_local_presenter: false },
  });

  assert(result.strategy === "text_fallback",
    `result.strategy must be text_fallback, got: ${result.strategy}`);
  assert(result.exitCode === 0, `exitCode must be 0, got: ${result.exitCode}`);
  assert(result.response.status === "text_fallback_pending",
    `response.status must be text_fallback_pending, got: ${result.response.status}`);
  // Pre-resolution: text_fallback does not write to response store
  const storedResp2 = readResponse(root, req.interaction_id);
  assert(storedResp2 === null,
    `response store must be empty for text_fallback pre-resolution, got: ${JSON.stringify(storedResp2)}`);

  fs.rmSync(root, { recursive: true, force: true });
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
  console.log("INTERACTION_FIXTURES_PASS");
}

main();
