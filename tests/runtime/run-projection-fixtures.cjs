#!/usr/bin/env node
"use strict";

// P1-N2: Runtime Schema Projection fixtures
// Tests for interaction block get/set/clear on session.yaml

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { writeSession, readSession } = require("../../scripts/runtime/store.cjs");

function makeTempRoot() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-projection-"));
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

// ─── RED: require projection module (will fail if not yet created) ───────────
const {
  getInteractionBlock,
  setInteractionBlock,
  clearInteractionBlock,
} = require("../../scripts/runtime/interaction-projection.cjs");

function makeBaseSession() {
  return {
    schema: 4,
    run_id: "wf-20260318-proj-001",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-p1", title: "Interaction projection test", mode: "feature" },
    phase: { current: "P1", status: "in_progress" },
    node: { active_id: "P1-N2", state: "red_pending", owner_persona: "author" },
    loop_budget: {
      max_nodes: 5,
      max_failures: 2,
      max_pending_approvals: 2,
      consumed_nodes: 0,
      consumed_failures: 0,
    },
    context_budget: {
      strategy: "hybrid",
      capsule_refresh_threshold: 4,
      summary_required_after_turns: 8,
    },
    workspace: { root: "D:/repo/demo", sandbox_mode: "workspace_write" },
    approvals: {
      pending: false,
      pending_count: 0,
      last_grant_scope: "none",
      last_approval_mode: "none",
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: null,
      restore_pending: false,
      restore_reason: null,
    },
    timestamps: {
      created_at: "2026-03-18T12:00:00Z",
      updated_at: "2026-03-18T12:00:00Z",
    },
  };
}

const cases = {};

// ─── CASE: getInteractionBlock returns null when no interaction block ─────────
cases["projection-get-null"] = function () {
  const session = makeBaseSession();
  const block = getInteractionBlock(session);
  assert(block === null, "must return null when no interaction block");
};

// ─── CASE: setInteractionBlock writes block to session.yaml ──────────────────
cases["projection-set-and-get"] = function () {
  const root = makeTempRoot();
  // Write base session without interaction block
  const session = makeBaseSession();
  writeSession(root, session);

  const block = {
    active_interaction_id: "ix-20260318-001",
    pending_count: 1,
    last_dispatched_at: null,
    blocking_kind: "approval",
    blocking_reason: "destructive_write_requires_approval",
  };
  setInteractionBlock(root, session, block);

  // Re-read session from disk
  const loaded = readSession(root);
  assert(loaded !== null, "session must be readable after setInteractionBlock");
  const got = getInteractionBlock(loaded);
  assert(got !== null, "interaction block must exist after set");
  assert(got.active_interaction_id === "ix-20260318-001", "active_interaction_id must match");
  assert(got.blocking_kind === "approval", "blocking_kind must match");
  assert(got.pending_count === 1, "pending_count must match");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: clearInteractionBlock removes the block ───────────────────────────
cases["projection-clear"] = function () {
  const root = makeTempRoot();
  const session = makeBaseSession();
  writeSession(root, session);

  const block = {
    active_interaction_id: "ix-20260318-001",
    pending_count: 1,
    last_dispatched_at: null,
    blocking_kind: "approval",
    blocking_reason: "destructive_write_requires_approval",
  };
  setInteractionBlock(root, session, block);
  const loaded1 = readSession(root);
  assert(getInteractionBlock(loaded1) !== null, "block must exist before clear");

  clearInteractionBlock(root, loaded1);
  const loaded2 = readSession(root);
  const got = getInteractionBlock(loaded2);
  assert(got === null, "interaction block must be null after clear");
  fs.rmSync(root, { recursive: true, force: true });
};

// ─── CASE: setInteractionBlock preserves other session fields ────────────────
cases["projection-preserves-session"] = function () {
  const root = makeTempRoot();
  const session = makeBaseSession();
  writeSession(root, session);

  const block = {
    active_interaction_id: "ix-20260318-002",
    pending_count: 1,
    last_dispatched_at: null,
    blocking_kind: "question",
    blocking_reason: "user_input_required",
  };
  setInteractionBlock(root, session, block);

  const loaded = readSession(root);
  assert(loaded.run_id === "wf-20260318-proj-001", "run_id must be preserved");
  assert(loaded.phase.current === "P1", "phase must be preserved");
  assert(loaded.node.active_id === "P1-N2", "node must be preserved");
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
  console.log("PROJECTION_FIXTURES_PASS");
}

main();
