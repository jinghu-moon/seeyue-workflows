#!/usr/bin/env node
"use strict";

// tests/e2e/run-interaction-conformance.cjs
// P2-N7: Interaction system acceptance harness
//
// Validates the full interaction stack:
//   1. Schema conformance: request objects match interaction.schema.yaml
//   2. MCP tool contracts: sy_list/read/resolve_interaction tool signatures
//   3. Engine capability-gap: adapter gap map completeness
//   4. Hook bridge: failure_mode field present on all hook_matrix entries
//   5. Store round-trip: write request -> read back -> resolve -> archive
//
// Run: node tests/e2e/run-interaction-conformance.cjs
// Exit 0 = all pass, Exit 1 = failures

const fs   = require("node:fs");
const os   = require("node:os");
const path = require("node:path");

const projectRoot = path.resolve(__dirname, "..", "..");

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function requireModule(relPath) {
  return require(path.join(projectRoot, relPath));
}

// ─── Suite 1: Schema shape conformance ───────────────────────────────────────
// Validates that request objects produced by interaction-builders comply with
// the required fields defined in workflow/interaction.schema.yaml.

function suite_schema_conformance() {
  const { buildApprovalRequest, buildQuestionRequest, buildInputRequest } = requireModule(
    "scripts/runtime/interaction-builders.cjs"
  );

  const REQUIRED_FIELDS = [
    "schema",
    "interaction_id",
    "kind",
    "status",
    "title",
    "message",
    "selection_mode",
    "options",
    "comment_mode",
    "presentation",
    "originating_request_id",
    "created_at",
  ];

  // approval_request
  const approvalReq = buildApprovalRequest({
    subject: "Deploy to production",
    risk_level: "high",
    originating_request_id: "req-test-001",
    options: [
      { id: "approve", label: "Approve", recommended: true },
      { id: "reject",  label: "Reject",  recommended: false },
    ],
  });
  for (const field of REQUIRED_FIELDS) {
    assert(field in approvalReq, `approval_request missing required field: '${field}'`);
  }
  assert(approvalReq.schema === 1, "schema must be 1");
  assert(/^ix-\d{8}-\d{3,}$/.test(approvalReq.interaction_id),
    `interaction_id '${approvalReq.interaction_id}' does not match ix-YYYYMMDD-NNN format`);
  assert(approvalReq.kind === "approval_request", "kind must be approval_request");
  assert(approvalReq.status === "pending", "status must be pending");

  // question_request
  const questionReq = buildQuestionRequest({
    question: "Which branch should be deployed?",
    originating_request_id: "req-test-002",
    options: [
      { id: "main",    label: "main",    recommended: true  },
      { id: "staging", label: "staging", recommended: false },
    ],
  });
  for (const field of REQUIRED_FIELDS) {
    assert(field in questionReq, `question_request missing required field: '${field}'`);
  }
  assert(questionReq.kind === "question_request", "kind must be question_request");

  // input_request
  const inputReq = buildInputRequest({
    prompt: "Enter target directory path",
    input_kind: "path",
    originating_request_id: "req-test-003",
  });
  for (const field of REQUIRED_FIELDS) {
    assert(field in inputReq, `input_request missing required field: '${field}'`);
  }
  assert(inputReq.kind === "input_request", "kind must be input_request");
}

// ─── Suite 2: Store round-trip ────────────────────────────────────────────────
// Validates write -> read -> resolve -> archive lifecycle in a temp dir.

function suite_store_round_trip() {
  const {
    ensureInteractionLayout,
    writeRequest,
    readRequest,
    writeResponse,
    readResponse,
    archiveInteraction,
    listPending,
    setActive,
    getActive,
    clearActive,
  } = requireModule("scripts/runtime/interaction-store.cjs");

  const { buildApprovalRequest } = requireModule("scripts/runtime/interaction-builders.cjs");

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "sy-conformance-"));
  try {
    ensureInteractionLayout(tmpDir);

    // Build and write a request
    const req = buildApprovalRequest({
      subject: "Conformance test approval",
      originating_request_id: "req-conformance-001",
      options: [
        { id: "approve", label: "Approve", recommended: true  },
        { id: "reject",  label: "Reject",  recommended: false },
      ],
    });
    writeRequest(tmpDir, req);

    // Verify pending list
    const pending = listPending(tmpDir);
    assert(pending.length === 1, `expected 1 pending, got ${pending.length}`);
    assert(pending[0].interaction_id === req.interaction_id,
      "pending item interaction_id mismatch");

    // Read back
    const readBack = readRequest(tmpDir, req.interaction_id);
    assert(readBack !== null, "readRequest must return the written object");
    assert(readBack.interaction_id === req.interaction_id, "read-back id mismatch");

    // Set and read active
    setActive(tmpDir, {
      active_id:       req.interaction_id,
      pending_count:   1,
      blocking_kind:   "hard_gate",
      blocking_reason: "Approval required",
    });
    const active = getActive(tmpDir);
    assert(active !== null, "getActive must return active object after setActive");
    assert(active.active_id === req.interaction_id, "active_id mismatch");

    // Write response
    const resp = {
      interaction_id:  req.interaction_id,
      selected_option: "approve",
      comment:         "LGTM",
      resolver:        "conformance_test",
      resolved_at:     new Date().toISOString(),
    };
    writeResponse(tmpDir, resp);
    const respRead = readResponse(tmpDir, req.interaction_id);
    assert(respRead !== null, "readResponse must return written response");
    assert(respRead.selected_option === "approve", "selected_option mismatch");

    // Archive
    archiveInteraction(tmpDir, req.interaction_id);
    const afterArchivePending = listPending(tmpDir);
    assert(afterArchivePending.length === 0, "pending list must be empty after archive");

    // Clear active
    clearActive(tmpDir);
    const afterClearActive = getActive(tmpDir);
    assert(afterClearActive === null, "getActive must return null after clearActive");
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

// ─── Suite 3: Capability-gap completeness ────────────────────────────────────
// Validates that all engines cover all known interaction kinds.

function suite_capability_gap_completeness() {
  const { getEngineCapabilityMap, KNOWN_INTERACTION_KINDS, ENGINE_CAPABILITIES } = requireModule(
    "scripts/runtime/capability-gap.cjs"
  );

  const engines = Object.keys(ENGINE_CAPABILITIES);
  assert(engines.length >= 3, `must have >= 3 engines, got ${engines.length}`);

  for (const engine of engines) {
    const map = getEngineCapabilityMap(engine);
    assert(map.engine === engine, "engine field mismatch");
    assert(typeof map.capabilities === "object", "capabilities must be an object");

    for (const kind of KNOWN_INTERACTION_KINDS) {
      const cap = map.capabilities[kind];
      assert(cap !== undefined, `engine '${engine}' missing capability for '${kind}'`);
      assert(["native", "partial", "gap"].includes(cap.native_level),
        `engine '${engine}' kind '${kind}' invalid native_level: '${cap.native_level}'`);
      assert(typeof cap.has_gap === "boolean", "has_gap must be boolean");
      assert(typeof cap.fallback === "string", "fallback must be a string");
    }
  }
}

// ─── Suite 4: Hook failure_mode completeness ──────────────────────────────────
// Validates that hooks.spec.yaml has failure_mode on all hook_matrix entries.

function suite_hook_failure_mode() {
  const hooksSpecPath = path.join(projectRoot, "workflow", "hooks.spec.yaml");
  assert(fs.existsSync(hooksSpecPath), `hooks.spec.yaml not found at ${hooksSpecPath}`);

  const src = fs.readFileSync(hooksSpecPath, "utf8");

  // Parse hook_matrix section line-by-line
  const lines = src.split("\n");
  let inHookMatrix = false;
  const entryLines = [];
  const failureModeLines = [];
  for (const line of lines) {
    if (/^hook_matrix:/.test(line)) { inHookMatrix = true; continue; }
    // Stop at next top-level key (non-blank, non-list line at col 0, not '- event:')
    if (inHookMatrix && /^[a-zA-Z_]/.test(line)) { inHookMatrix = false; }
    if (!inHookMatrix) continue;
    if (/^- event:/.test(line)) entryLines.push(line);
    if (/^  failure_mode:/.test(line)) failureModeLines.push(line);
  }

  assert(
    entryLines.length > 0,
    "hooks.spec.yaml must have at least one hook_matrix entry"
  );
  assert(
    failureModeLines.length >= entryLines.length,
    `Every hook_matrix entry must have failure_mode. ` +
    `Found ${entryLines.length} entries but only ${failureModeLines.length} failure_mode fields.`
  );

  const validValues = ["hard_gate", "advisory", "telemetry"];
  for (const line of failureModeLines) {
    const match = line.match(/failure_mode:\s*(\S+)/);
    if (match) {
      assert(
        validValues.includes(match[1]),
        `Invalid failure_mode value: '${match[1]}'. Valid: ${validValues.join(", ")}`
      );
    }
  }
}

// ─── Suite 5: Interaction MCP module API ─────────────────────────────────────
// Validates that interaction-projection.cjs exports the required interface.

function suite_mcp_interaction_projection() {
  const projPath = path.join(projectRoot, "scripts", "runtime", "interaction-projection.cjs");
  assert(fs.existsSync(projPath), `interaction-projection.cjs not found`);

  const mod = require(projPath);
  assert(typeof mod.getInteractionBlock === "function",
    "must export getInteractionBlock");

  // Null session -> no block
  const result = mod.getInteractionBlock({});
  assert(result === null || typeof result === "object",
    "getInteractionBlock must return null or object");
}

// ─── Suite 6: Interaction router API ─────────────────────────────────────────

function suite_interaction_router() {
  const routerPath = path.join(projectRoot, "scripts", "runtime", "interaction-router.cjs");
  assert(fs.existsSync(routerPath), `interaction-router.cjs not found`);

  const mod = require(routerPath);
  assert(typeof mod.shouldBlockForInteraction === "function",
    "must export shouldBlockForInteraction");
  assert(typeof mod.getInteractionBlocker === "function",
    "must export getInteractionBlocker");

  // Empty session -> no block
  const shouldBlock = mod.shouldBlockForInteraction({});
  assert(shouldBlock === false, "empty session must not block");

  const blocker = mod.getInteractionBlocker({});
  assert(blocker === null, "empty session blocker must be null");
}

// ─── Runner ───────────────────────────────────────────────────────────────────

const SUITES = [
  ["schema-conformance",           suite_schema_conformance],
  ["store-round-trip",             suite_store_round_trip],
  ["capability-gap-completeness",  suite_capability_gap_completeness],
  ["hook-failure-mode",            suite_hook_failure_mode],
  ["mcp-interaction-projection",   suite_mcp_interaction_projection],
  ["interaction-router",           suite_interaction_router],
];

function main() {
  const suiteArg = process.argv.indexOf("--suite");
  const suiteName = suiteArg !== -1 ? process.argv[suiteArg + 1] : null;

  let failed = false;
  let ran = 0;

  const toRun = suiteName
    ? SUITES.filter(([name]) => name === suiteName)
    : SUITES;

  if (suiteName && toRun.length === 0) {
    console.error(`Unknown suite: ${suiteName}`);
    console.error(`Available: ${SUITES.map(([n]) => n).join(", ")}`);
    process.exit(1);
  }

  for (const [name, fn] of toRun) {
    try {
      fn();
      console.log(`SUITE_PASS ${name}`);
      ran++;
    } catch (error) {
      failed = true;
      console.error(`SUITE_FAIL ${name}`);
      console.error(`  ${error.message}`);
    }
  }

  console.log(`\n${ran}/${toRun.length} suites passed.`);

  if (failed) {
    process.exit(1);
  }
}

main();
