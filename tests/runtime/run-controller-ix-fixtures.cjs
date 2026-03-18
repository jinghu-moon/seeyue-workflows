#!/usr/bin/env node
"use strict";

// P1-N6: Controller/Router Integration fixtures
// Tests that a session with active interaction blocks normal advance

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { writeSession, readSession } = require("../../scripts/runtime/store.cjs");
const { setInteractionBlock } = require("../../scripts/runtime/interaction-projection.cjs");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-controller-ix-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── RED: require interaction-router module ───────────────────────────────────
const {
  shouldBlockForInteraction,
  getInteractionBlocker,
} = require("../../scripts/runtime/interaction-router.cjs");

function makeBaseSession() {
  return {
    schema: 4,
    run_id: "wf-20260318-ctrl-001",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-p1", title: "Controller integration test", mode: "feature" },
    phase: { current: "P1", status: "in_progress" },
    node: { active_id: "P1-N6", state: "red_pending", owner_persona: "author" },
    loop_budget: {
      max_nodes: 5, max_failures: 2, max_pending_approvals: 2,
      consumed_nodes: 0, consumed_failures: 0,
    },
    context_budget: { strategy: "hybrid", capsule_refresh_threshold: 4, summary_required_after_turns: 8 },
    workspace: { root: "D:/repo/demo", sandbox_mode: "workspace_write" },
    approvals: {
      pending: false, pending_count: 0, last_grant_scope: "none",
      last_approval_mode: "none", active_request: null, grants: [],
    },
    recovery: { last_checkpoint_id: null, restore_pending: false, restore_reason: null },
    timestamps: { created_at: "2026-03-18T12:00:00Z", updated_at: "2026-03-18T12:00:00Z" },
  };
}

const cases = {};

// ─── CASE: session without interaction block does not block ──────────────────
cases["router-no-interaction-no-block"] = function () {
  const session = makeBaseSession();
  assert(!shouldBlockForInteraction(session), "session without interaction block must not block");
};

// ─── CASE: session with active interaction blocks ────────────────────────────
cases["router-active-interaction-blocks"] = function () {
  const session = Object.assign({}, makeBaseSession(), {
    interaction: {
      active_interaction_id: "ix-20260318-001",
      pending_count: 1,
      last_dispatched_at: null,
      blocking_kind: "approval",
      blocking_reason: "destructive_write_requires_approval",
    },
  });
  assert(shouldBlockForInteraction(session), "session with active interaction must block");
};

// ─── CASE: getInteractionBlocker returns descriptor ───────────────────────────
cases["router-blocker-descriptor"] = function () {
  const session = Object.assign({}, makeBaseSession(), {
    interaction: {
      active_interaction_id: "ix-20260318-002",
      pending_count: 1,
      last_dispatched_at: null,
      blocking_kind: "restore",
      blocking_reason: "journal_orphan_detected",
    },
  });
  const blocker = getInteractionBlocker(session);
  assert(blocker !== null, "blocker must not be null when interaction is active");
  assert(blocker.type === "interaction_pending", `blocker.type must be interaction_pending, got ${blocker.type}`);
  assert(blocker.interaction_id === "ix-20260318-002", "interaction_id must match");
  assert(blocker.blocking_kind === "restore", "blocking_kind must match");
  assert(typeof blocker.reason === "string", "reason must be string");
};

// ─── CASE: getInteractionBlocker returns null when no active interaction ──────
cases["router-no-blocker-when-no-interaction"] = function () {
  const session = makeBaseSession();
  const blocker = getInteractionBlocker(session);
  assert(blocker === null, "blocker must be null when no active interaction");
};

// ─── CASE: null/undefined interaction.active_interaction_id does not block ───
cases["router-null-active-id-no-block"] = function () {
  const session = Object.assign({}, makeBaseSession(), {
    interaction: { active_interaction_id: null, pending_count: 0, last_dispatched_at: null },
  });
  assert(!shouldBlockForInteraction(session), "null active_interaction_id must not block");
};

// ─── Runner ──────────────────────────────────────────────────────────────────
function parseArgs(argv) {
  const parsed = { caseName: null };
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--case") { i++; parsed.caseName = argv[i]; continue; }
    throw new Error(`Unknown argument: ${argv[i]}`);
  }
  return parsed;
}

function main() {
  const parsed = parseArgs(process.argv.slice(2));
  const selected = parsed.caseName ? [[parsed.caseName, cases[parsed.caseName]]] : Object.entries(cases);
  if (selected.some(([, run]) => typeof run !== "function")) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`); process.exit(1);
  }
  let failed = false;
  for (const [caseName, run] of selected) {
    try { run(); console.log(`CASE_PASS ${caseName}`); }
    catch (error) { failed = true; console.error(`CASE_FAIL ${caseName}`); console.error(error.stack || error.message); }
  }
  if (failed) process.exit(1);
  console.log("CONTROLLER_IX_FIXTURES_PASS");
}

main();
