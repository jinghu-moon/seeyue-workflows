#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const { appendJournalEvents, readCheckpoint, readJournalEvents, readSession, writeSession, writeSprintStatus, writeTaskGraph } = require("../../scripts/runtime/store.cjs");
const { loadWorkflowState } = require("../../scripts/hooks/sy-hook-lib.cjs");
const {
  makeBaseSession,
  makeBaseSprintStatus,
  makeBaseTaskGraph,
  makeTempRoot,
} = require("../runtime/runtime-fixture-lib.cjs");

const projectRoot = path.resolve(__dirname, "..", "..");
const pretoolBashHook = path.join(projectRoot, "scripts", "hooks", "sy-pretool-bash.cjs");
const pretoolBashBudgetHook = path.join(projectRoot, "scripts", "hooks", "sy-pretool-bash-budget.cjs");
const posttoolBashVerifyHook = path.join(projectRoot, "scripts", "hooks", "sy-posttool-bash-verify.cjs");
const posttoolWriteHook = path.join(projectRoot, "scripts", "hooks", "sy-posttool-write.cjs");
const pretoolWriteHook = path.join(projectRoot, "scripts", "hooks", "sy-pretool-write.cjs");
const stopHook = path.join(projectRoot, "scripts", "hooks", "sy-stop.cjs");

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

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

function writeLegacyFlatSession(rootDir) {
  const sessionPath = path.join(rootDir, ".ai", "workflow", "session.yaml");
  fs.mkdirSync(path.dirname(sessionPath), { recursive: true });
  fs.writeFileSync(
    sessionPath,
    [
      "run_id: wf-20260308-301",
      "current_phase: execute",
      "next_action: /execute node P3-N1",
      "tdd_required: true",
      "red_verified: false",
      "target: scripts/runtime/hook-client.cjs",
      "updated_at: 2026-03-08T12:00:00Z",
      "",
    ].join("\n"),
    "utf8",
  );
}

function runHook(rootDir, scriptPath, payload) {
  const result = spawnSync(process.execPath, [scriptPath], {
    cwd: rootDir,
    input: JSON.stringify(payload),
    encoding: "utf8",
  });
  return {
    code: Number.isInteger(result.status) ? result.status : 1,
    stdout: String(result.stdout || "").trim(),
    stderr: String(result.stderr || "").trim(),
  };
}

function parseHookOutput(stdout) {
  try {
    const parsed = JSON.parse(stdout || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function buildStructuredRuntime(rootDir, overrides = {}) {
  const session = makeBaseSession();
  const taskGraph = makeBaseTaskGraph();
  const sprintStatus = makeBaseSprintStatus();

  session.engine.kind = "codex";
  session.run_id = "wf-20260308-302";
  session.phase.current = "P3";
  session.phase.status = "in_progress";
  session.node.active_id = "P3-N1";
  session.node.state = "red_pending";
  session.node.owner_persona = "author";
  session.approvals.pending = true;
  session.approvals.pending_count = 1;
  session.approvals.active_request = {
    approval_id: "apr-001",
    action: "write_file",
    target_ref: "scripts/hooks/sy-hook-lib.cjs",
    risk_class: "high",
    approval_mode: "manual_required",
    grant_scope: "once",
    status: "pending",
    requested_at: "2026-03-08T12:00:00Z",
    expires_at: null,
    decision: null,
  };
  session.recovery.restore_pending = false;
  session.timestamps.updated_at = "2026-03-08T12:05:00Z";

  if (overrides.session && typeof overrides.session === "object") {
    Object.assign(session, overrides.session);
  }
  if (overrides.phase && typeof overrides.phase === "object") {
    session.phase = { ...session.phase, ...overrides.phase };
  }
  if (overrides.node && typeof overrides.node === "object") {
    session.node = { ...session.node, ...overrides.node };
  }
  if (overrides.approvals && typeof overrides.approvals === "object") {
    session.approvals = { ...session.approvals, ...overrides.approvals };
  }
  if (overrides.recovery && typeof overrides.recovery === "object") {
    session.recovery = { ...session.recovery, ...overrides.recovery };
  }

  taskGraph.phases = [
    {
      id: "P2",
      title: "Router And Policy",
      status: "completed",
      depends_on: [],
      entry_condition: ["P1 completed"],
      exit_gate: { cmd: "npm run test:runtime:p2", pass_signal: "ENGINE_KERNEL_PASS", coverage_min: "90%" },
      rollback_boundary: { revert_nodes: ["P2-N1"], restore_point: "P1 stable" },
    },
    {
      id: "P3",
      title: "Hooks",
      status: "in_progress",
      depends_on: ["P2"],
      entry_condition: ["P2 completed"],
      exit_gate: { cmd: "node tests/hooks/run-v4-fixtures.cjs", pass_signal: "HOOKS_V4_FIXTURES_PASS", coverage_min: "90%" },
      rollback_boundary: { revert_nodes: ["P3-N1"], restore_point: "P2 stable" },
    },
  ];

  taskGraph.nodes = [
    {
      id: "P3-N1",
      phase_id: "P3",
      title: "Shared runtime client",
      target: "scripts/runtime/hook-client.cjs",
      action: "Unify hooks runtime snapshot access",
      why: "Hooks must use structured V4 state instead of flat legacy fields",
      depends_on: [],
      verify: {
        cmd: "node tests/hooks/run-v4-fixtures.cjs --case shared-runtime-client",
        pass_signal: "HOOK_RUNTIME_CLIENT_PASS",
      },
      risk_level: "high",
      tdd_required: true,
      status: "in_progress",
      tdd_state: "red_pending",
      owner_persona: "author",
      review_state: { spec_review: "pending", quality_review: "pending" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: "apr-001",
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: {
        layer: "unit",
        coverage_mode: "full",
        coverage_profile: "standard",
        mock_policy: "boundary_only",
        acceptance_criteria_refs: ["AC-P3-N1-1"],
        red_cmd: "node tests/hooks/run-v4-fixtures.cjs --case legacy-flat-session-only",
        green_cmd: "node tests/hooks/run-v4-fixtures.cjs --case shared-runtime-client",
        red_expectation: {
          allowed_failure_kinds: ["assertion_failure"],
          rejected_failure_kinds: ["syntax_error", "environment_error"],
          allowed_exit_codes: [1],
          stderr_pattern: null,
          error_type: null,
        },
        behavior_gate: {
          ac_traceability_required: true,
          boundary_conditions_required: false,
        },
      },
    },
  ];

  if (overrides.activeNode && typeof overrides.activeNode === "object") {
    taskGraph.nodes[0] = { ...taskGraph.nodes[0], ...overrides.activeNode };
  }

  sprintStatus.active_phase = "P3";
  sprintStatus.node_summary = [
    {
      id: "P3-N1",
      status: taskGraph.nodes[0].status,
      tdd_state: taskGraph.nodes[0].tdd_state,
    },
  ];
  sprintStatus.recommended_next = [
    {
      type: "request_approval",
      target: "P3-N1",
      params: { approval_id: "apr-001", persona: "human" },
      reason: "resolve pending approval",
      blocking_on: ["approval_pending"],
      priority: "now",
    },
  ];

  if (overrides.sprintStatus && typeof overrides.sprintStatus === "object") {
    Object.assign(sprintStatus, overrides.sprintStatus);
  }

  writeSession(rootDir, session);
  writeTaskGraph(rootDir, taskGraph);
  writeSprintStatus(rootDir, sprintStatus);
  appendJournalEvents(rootDir, [
    {
      ts: "2026-03-08T12:05:00Z",
      event: "node_started",
      node_id: "P3-N1",
      phase_id: "P3",
    },
  ]);

  if (Array.isArray(overrides.journalEvents) && overrides.journalEvents.length > 0) {
    appendJournalEvents(rootDir, overrides.journalEvents);
  }
}

function runLegacyFlatSessionOnly() {
  const rootDir = makeTempRoot("sy-hooks-v4-legacy-");
  writeLegacyFlatSession(rootDir);

  const state = loadWorkflowState(rootDir);
  assert(state.sourceModel === "legacy_flat", `expected sourceModel=legacy_flat but got ${JSON.stringify(state.sourceModel)}`);
  assert(state.runtimeReady === false, `expected runtimeReady=false but got ${JSON.stringify(state.runtimeReady)}`);
  assert(state.phase === "execute", `expected legacy phase=execute but got ${JSON.stringify(state.phase)}`);
}

function runSharedRuntimeClient() {
  const {
    getActiveNode,
    getActivePhase,
    getRecommendedNext,
    isApprovalPending,
    isRestorePending,
    loadRuntimeSnapshot,
  } = require("../../scripts/runtime/hook-client.cjs");

  const rootDir = makeTempRoot("sy-hooks-v4-runtime-");
  buildStructuredRuntime(rootDir);

  const snapshot = loadRuntimeSnapshot(rootDir);
  assert(snapshot.complete === true, `expected complete=true but got ${JSON.stringify(snapshot.complete)}`);
  assert(snapshot.valid.session === true, "expected valid.session=true");
  assert(snapshot.valid.taskGraph === true, "expected valid.taskGraph=true");
  assert(snapshot.valid.sprintStatus === true, "expected valid.sprintStatus=true");
  assert(snapshot.exists.journal === true, "expected journal file to exist");

  const activePhase = getActivePhase(snapshot);
  const activeNode = getActiveNode(snapshot);
  const recommendedNext = getRecommendedNext(snapshot);

  assert(activePhase && activePhase.id === "P3", `expected active phase P3 but got ${JSON.stringify(activePhase && activePhase.id)}`);
  assert(activeNode && activeNode.id === "P3-N1", `expected active node P3-N1 but got ${JSON.stringify(activeNode && activeNode.id)}`);
  assert(Array.isArray(recommendedNext) && recommendedNext[0] && recommendedNext[0].type === "request_approval", "expected recommended_next[0].type=request_approval");
  assert(isApprovalPending(snapshot) === true, "expected approval pending=true");
  assert(isRestorePending(snapshot) === false, "expected restore pending=false");

  const state = loadWorkflowState(rootDir);
  assert(state.sourceModel === "v4_runtime", `expected sourceModel=v4_runtime but got ${JSON.stringify(state.sourceModel)}`);
  assert(state.runtimeReady === true, `expected runtimeReady=true but got ${JSON.stringify(state.runtimeReady)}`);
  assert(state.phase === "execute", `expected projected phase=execute but got ${JSON.stringify(state.phase)}`);
  assert(state.phaseId === "P3", `expected phaseId=P3 but got ${JSON.stringify(state.phaseId)}`);
  assert(state.activeNodeId === "P3-N1", `expected activeNodeId=P3-N1 but got ${JSON.stringify(state.activeNodeId)}`);
  assert(state.fields.tdd_required === "true", `expected tdd_required=true but got ${JSON.stringify(state.fields.tdd_required)}`);
  assert(state.fields.red_verified === "false", `expected red_verified=false but got ${JSON.stringify(state.fields.red_verified)}`);
  assert(state.nextAction === "request_approval:p3-n1", `expected normalized nextAction=request_approval:p3-n1 but got ${JSON.stringify(state.nextAction)}`);
}

function runInvalidRedAllowsWrite() {
  const rootDir = makeTempRoot("sy-hooks-v4-invalid-red-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    node: {
      state: "green_pending",
    },
    activeNode: {
      status: "in_progress",
      tdd_state: "green_pending",
      approval_ref: null,
    },
    journalEvents: [
      {
        ts: "2026-03-08T12:06:00Z",
        event: "red_recorded",
        node_id: "P3-N1",
        payload: {
          executed: true,
          testFailed: true,
          failureKind: "environment_error",
          exitCode: 1,
          recorded: true,
        },
      },
    ],
  });

  const result = runHook(rootDir, pretoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: "src/app.ts",
      content: "export const value = 1;\n",
    },
  });

  assert(result.code === 2, `expected hook to block invalid red but got exit=${result.code}`);
  assert(/invalid_red|RED|绾㈢伅|澶辫触娴嬭瘯/i.test(result.stderr), `expected invalid red hint in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runPrewriteRedGate() {
  const rootDir = makeTempRoot("sy-hooks-v4-prewrite-red-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    node: {
      state: "red_pending",
    },
    activeNode: {
      status: "in_progress",
      tdd_state: "red_pending",
      approval_ref: null,
    },
    sprintStatus: {
      recommended_next: [
        {
          type: "resume_node",
          target: "P3-N1",
          params: { persona: "author" },
          reason: "complete red gate first",
          blocking_on: ["red_pending"],
          priority: "now",
        },
      ],
    },
  });

  const result = runHook(rootDir, pretoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: "src/app.ts",
      content: "export const value = 1;\n",
    },
  });

  assert(result.code === 2, `expected hook to block before green code write but got exit=${result.code}`);
  assert(/RED|澶辫触娴嬭瘯|鍏堣ˉ/i.test(result.stderr), `expected red gate message but got ${JSON.stringify(result.stderr)}`);
}

function runApprovalRequestZhCn() {
  const rootDir = makeTempRoot("sy-hooks-v4-approval-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    node: {
      state: "verified",
    },
    activeNode: {
      status: "in_progress",
      tdd_required: false,
      tdd_state: "not_applicable",
      approval_ref: null,
    },
  });

  const result = runHook(rootDir, pretoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: "scripts/hooks/sy-pretool-write.cjs",
      content: "module.exports = {};\n",
    },
  });

  assert(result.code === 2, `expected approval gate block but got exit=${result.code}`);
  assert(result.stderr.includes("需要人工审批"), `expected zh-CN approval headline but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("操作类型：写入文件"), `expected operation description but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("风险等级：关键 (critical)"), `expected critical risk in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runApprovalRequestZhCnV2() {
  const rootDir = makeTempRoot("sy-hooks-v4-approval-v2-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    node: {
      state: "verified",
    },
    activeNode: {
      status: "in_progress",
      tdd_required: false,
      tdd_state: "not_applicable",
      approval_ref: null,
    },
  });

  const result = runHook(rootDir, pretoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: "scripts/hooks/sy-pretool-write.cjs",
      content: "module.exports = {};\n",
    },
  });

  assert(result.code === 2, `expected approval gate block but got exit=${result.code}`);
  assert(result.stderr.includes("需要人工审批"), `expected zh-CN approval headline but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("操作类型：写入文件"), `expected operation description but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("风险等级：关键 (critical)"), `expected critical risk in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runStopManualRestoreRequiresHuman() {
  const rootDir = makeTempRoot("sy-hooks-v4-stop-manual-restore-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "node-restore-003",
      restore_pending: true,
      restore_reason: "missing_tool_call_metadata",
    },
    sprintStatus: {
      recommended_next: [
        {
          type: "human_intervention",
          target: "P3-N1",
          params: {},
          reason: "resolve restore blocker before resume",
          blocking_on: ["restore_requires_human"],
          priority: "now",
        },
      ],
    },
  });

  const result = runHook(rootDir, stopHook, {
    cwd: rootDir,
  });
  const output = parseHookOutput(result.stdout);

  assert(result.code === 0, `expected stop gate force_continue exit 0 but got exit=${result.code}`);
  assert(output.verdict === "force_continue", `expected verdict=force_continue but got ${JSON.stringify(output)}`);
  assert(result.stderr.includes("恢复未完成，当前轮次不能结束"), `expected restore block headline but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("需要人工处理恢复"), `expected restore guidance headline but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("恢复原因：缺少工具调用元数据，无法自动恢复 (missing_tool_call_metadata)"), `expected restore reason label but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("检查点：node-restore-003"), `expected checkpoint id in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runBudgetExhausted() {
  const rootDir = makeTempRoot("sy-hooks-v4-budget-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      restore_pending: false,
      restore_reason: null,
    },
    session: {
      loop_budget: {
        max_nodes: 2,
        max_failures: 2,
        max_pending_approvals: 1,
        consumed_nodes: 2,
        consumed_failures: 0,
      },
    },
    sprintStatus: {
      recommended_next: [
        {
          type: "human_intervention",
          target: "P3",
          params: { reason: "budget exhausted" },
          reason: "budget exhausted",
          blocking_on: ["budget_exhausted"],
          priority: "now",
        },
      ],
    },
  });

  const result = runHook(rootDir, pretoolBashBudgetHook, {
    cwd: rootDir,
    tool_name: "Bash",
    tool_input: {
      command: "npm test",
    },
  });

  assert(result.code === 2, `expected budget gate block but got exit=${result.code}`);
  assert(/budget|max_nodes|consumed_nodes/i.test(result.stderr), `expected budget details in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runStopWithoutResumeFrontier() {
  const rootDir = makeTempRoot("sy-hooks-v4-stop-no-frontier-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "node-restore-001",
      restore_pending: true,
      restore_reason: "missing_terminal_event",
    },
    sprintStatus: {
      recommended_next: [],
    },
  });

  const result = runHook(rootDir, stopHook, {
    cwd: rootDir,
  });
  const output = parseHookOutput(result.stdout);

  assert(result.code === 0, `expected stop gate force_continue exit 0 but got exit=${result.code}`);
  assert(output.verdict === "force_continue", `expected verdict=force_continue but got ${JSON.stringify(output)}`);
  assert(/resume|restore|frontier|missing_terminal_event/i.test(result.stderr), `expected resume frontier hint in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runStopRequiresResumeFrontier() {
  const rootDir = makeTempRoot("sy-hooks-v4-stop-frontier-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "node-restore-002",
      restore_pending: true,
      restore_reason: "missing_terminal_event",
    },
    sprintStatus: {
      recommended_next: [
        {
          type: "resume_node",
          target: "P3-N1",
          params: { persona: "author" },
          reason: "restore interrupted node",
          blocking_on: ["restore_pending"],
          priority: "now",
        },
      ],
    },
  });

  const result = runHook(rootDir, stopHook, {
    cwd: rootDir,
  });
  const output = parseHookOutput(result.stdout);

  assert(result.code === 0, `expected stop gate force_continue exit 0 but got exit=${result.code}`);
  assert(output.verdict === "force_continue", `expected verdict=force_continue but got ${JSON.stringify(output)}`);
  assert(/resume_node|P3-N1|restore_pending|resume frontier/i.test(result.stderr), `expected structured resume frontier in stderr but got ${JSON.stringify(result.stderr)}`);
}

function runMissingJournalEventAfterWrite() {
  const rootDir = makeTempRoot("sy-hooks-v4-postwrite-missing-journal-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "ckpt-postwrite-001",
    },
  });

  const filePath = "scripts/runtime/hook-client.cjs";
  const absolutePath = path.join(rootDir, filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, "module.exports = {}\n", "utf8");

  const result = runHook(rootDir, posttoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: filePath,
      content: 'module.exports = { value: 1 };\n',
    },
  });

  const journal = readJournalEvents(rootDir);
  const latest = journal[journal.length - 1] || null;

  assert(result.code === 0, `expected post write hook allow but got exit=${result.code}`);
  assert(latest && latest.event === "write_recorded", `expected latest journal event write_recorded but got ${JSON.stringify(latest)}`);
}

function runPostwriteJournalAppend() {
  const rootDir = makeTempRoot("sy-hooks-v4-postwrite-journal-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "ckpt-postwrite-002",
    },
  });

  const filePath = "scripts/runtime/hook-client.cjs";
  const absolutePath = path.join(rootDir, filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, "module.exports = {}\n", "utf8");

  const result = runHook(rootDir, posttoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: filePath,
      content: 'module.exports = { value: 2 };\n',
    },
  });

  const journal = readJournalEvents(rootDir);
  const latest = journal[journal.length - 1] || null;

  assert(result.code === 0, `expected post write hook allow but got exit=${result.code}`);
  assert(latest && latest.event === "write_recorded", `expected write_recorded event but got ${JSON.stringify(latest)}`);
  assert(latest.node_id === "P3-N1", `expected node_id=P3-N1 but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.file === filePath, `expected file payload but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.checkpoint_id === "ckpt-postwrite-002", `expected checkpoint metadata but got ${JSON.stringify(latest)}`);
}

function runVerifyEvidenceCapture() {
  const rootDir = makeTempRoot("sy-hooks-v4-verify-evidence-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: "ckpt-verify-001",
    },
  });

  const result = runHook(rootDir, posttoolBashVerifyHook, {
    cwd: rootDir,
    tool_name: "Bash",
    tool_input: {
      command: "npm test",
    },
    tool_response: {
      returncode: 0,
      stdout: "test result: ok. 3 passed; 0 failed;",
      stderr: "",
    },
  });

  const journal = readJournalEvents(rootDir);
  const latest = journal[journal.length - 1] || null;
  const stagingPath = path.join(rootDir, ".ai", "analysis", "verify-staging.json");

  assert(result.code === 0, `expected verify capture allow but got exit=${result.code}`);
  assert(fs.existsSync(stagingPath), `expected verify staging file at ${stagingPath}`);
  assert(latest && latest.event === "verification_recorded", `expected verification_recorded event but got ${JSON.stringify(latest)}`);
  assert(latest.node_id === "P3-N1", `expected verification node_id=P3-N1 but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.command === "npm test", `expected command payload but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.status === "pass", `expected pass status but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.checkpoint_id === "ckpt-verify-001", `expected checkpoint_id metadata but got ${JSON.stringify(latest)}`);
}

function runPretoolWriteCreatesPreDestructiveCheckpoint() {
  const rootDir = makeTempRoot("sy-hooks-v4-prewrite-checkpoint-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
    recovery: {
      last_checkpoint_id: null,
    },
    activeNode: {
      tdd_required: false,
      tdd_state: "not_applicable",
    },
  });

  const filePath = "scripts/runtime/hook-client.cjs";
  const absolutePath = path.join(rootDir, filePath);
  fs.mkdirSync(path.dirname(absolutePath), { recursive: true });
  fs.writeFileSync(absolutePath, "module.exports = { before: true };\n", "utf8");

  const result = runHook(rootDir, pretoolWriteHook, {
    cwd: rootDir,
    tool_name: "Write",
    tool_input: {
      file_path: filePath,
      content: "module.exports = { after: true };\n",
    },
  });

  const session = readSession(rootDir);
  const checkpointId = session?.recovery?.last_checkpoint_id;
  const checkpoint = checkpointId ? readCheckpoint(rootDir, checkpointId) : null;
  const journal = readJournalEvents(rootDir);

  assert(result.code === 0, `expected pretool write allow but got exit=${result.code} stderr=${JSON.stringify(result.stderr)}`);
  assert(checkpoint && checkpoint.checkpoint_class === "pre_destructive", `expected pre_destructive checkpoint but got ${JSON.stringify(checkpoint)}`);
  assert(checkpoint.target_ref === filePath, `expected checkpoint target_ref=${filePath} but got ${JSON.stringify(checkpoint?.target_ref)}`);
  assert(checkpoint.operation_kind === "write", `expected operation_kind=write but got ${JSON.stringify(checkpoint?.operation_kind)}`);
  assert(checkpoint.target_snapshot_ref, `expected target_snapshot_ref but got ${JSON.stringify(checkpoint)}`);
  assert(checkpoint.target_snapshot_content_ref, `expected target_snapshot_content_ref but got ${JSON.stringify(checkpoint)}`);
  assert(journal.some((item) => item.event === "checkpoint_created" && item.payload?.checkpoint_class === "pre_destructive"), "expected checkpoint_created event for pre-destructive checkpoint");
}

function runPretoolBashCreatesPreDestructiveCheckpoint() {
  const rootDir = makeTempRoot("sy-hooks-v4-prebash-checkpoint-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [
        {
          approval_id: "apr-bash-001",
          grant_scope: "once",
          approval_mode: "manual_required",
          action: "bash",
          target_ref: "git_mutating",
          risk_class: "high",
          decision: "approved",
          granted_at: "2026-03-08T12:06:00Z",
          expires_at: null,
        },
      ],
    },
    recovery: {
      last_checkpoint_id: null,
    },
  });

  const command = "git add scripts/runtime/hook-client.cjs";
  const result = runHook(rootDir, pretoolBashHook, {
    cwd: rootDir,
    tool_name: "Bash",
    tool_input: {
      command,
    },
  });

  const session = readSession(rootDir);
  const checkpointId = session?.recovery?.last_checkpoint_id;
  const checkpoint = checkpointId ? readCheckpoint(rootDir, checkpointId) : null;
  const journal = readJournalEvents(rootDir);

  assert(result.code === 0, `expected pretool bash allow but got exit=${result.code} stderr=${JSON.stringify(result.stderr)}`);
  assert(checkpoint && checkpoint.checkpoint_class === "pre_destructive", `expected pre_destructive checkpoint but got ${JSON.stringify(checkpoint)}`);
  assert(checkpoint.command_class === "git_mutating", `expected command_class=git_mutating but got ${JSON.stringify(checkpoint?.command_class)}`);
  assert(checkpoint.operation_kind === "bash", `expected operation_kind=bash but got ${JSON.stringify(checkpoint?.operation_kind)}`);
  assert(checkpoint.target_ref === command, `expected target_ref command but got ${JSON.stringify(checkpoint?.target_ref)}`);
  assert(checkpoint.target_snapshot_ref === null, `expected no file snapshot for bash command checkpoint but got ${JSON.stringify(checkpoint?.target_snapshot_ref)}`);
  assert(journal.some((item) => item.event === "checkpoint_created" && item.payload?.checkpoint_class === "pre_destructive"), "expected checkpoint_created event for bash checkpoint");
}

function runPrebashApprovalGate() {
  const rootDir = makeTempRoot("sy-hooks-v4-prebash-approval-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
  });

  const result = runHook(rootDir, pretoolBashHook, {
    cwd: rootDir,
    tool_name: "Bash",
    tool_input: {
      command: "curl https://example.com",
    },
  });

  assert(result.code === 2, `expected approval gate block but got exit=${result.code}`);
  assert(result.stderr.includes("需要人工审批"), `expected approval headline but got ${JSON.stringify(result.stderr)}`);
  assert(result.stderr.includes("操作类型：执行命令"), `expected approval action label but got ${JSON.stringify(result.stderr)}`);
}

function runRedEvidenceCapture() {
  const rootDir = makeTempRoot("sy-hooks-v4-red-evidence-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
  });

  const command = "node tests/hooks/run-v4-fixtures.cjs --case legacy-flat-session-only";
  const result = runHook(rootDir, posttoolBashVerifyHook, {
    cwd: rootDir,
    tool_name: "Bash",
    tool_input: {
      command,
    },
    tool_response: {
      returncode: 1,
      stdout: "AssertionError: expected value to equal 2",
      stderr: "",
    },
  });

  const journal = readJournalEvents(rootDir);
  const latest = [...journal].reverse().find((entry) => entry?.event === "red_recorded");

  assert(result.code === 0, `expected red capture allow but got exit=${result.code}`);
  assert(latest && latest.event === "red_recorded", `expected red_recorded event but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.testFailed === true, `expected testFailed=true but got ${JSON.stringify(latest?.payload)}`);
  assert(latest.payload && latest.payload.failureKind === "assertion_failure", `expected assertion_failure but got ${JSON.stringify(latest?.payload)}`);
  assert(latest.payload && latest.payload.exitCode === 1, `expected exitCode=1 but got ${JSON.stringify(latest?.payload)}`);
}

function runGreenEvidenceCapture() {
  const rootDir = makeTempRoot("sy-hooks-v4-green-evidence-");
  buildStructuredRuntime(rootDir, {
    approvals: {
      pending: false,
      pending_count: 0,
      active_request: null,
      grants: [],
    },
  });

  const command = "node tests/hooks/run-v4-fixtures.cjs --case shared-runtime-client";
  const result = runHook(rootDir, posttoolBashVerifyHook, {
    cwd: rootDir,
    tool_name: "Bash",
    tool_input: {
      command,
    },
    tool_response: {
      returncode: 0,
      stdout: "all tests passed",
      stderr: "",
    },
  });

  const journal = readJournalEvents(rootDir);
  const latest = [...journal].reverse().find((entry) => entry?.event === "green_recorded");

  assert(result.code === 0, `expected green capture allow but got exit=${result.code}`);
  assert(latest && latest.event === "green_recorded", `expected green_recorded event but got ${JSON.stringify(latest)}`);
  assert(latest.payload && latest.payload.passed === true, `expected passed=true but got ${JSON.stringify(latest?.payload)}`);
  assert(latest.payload && latest.payload.exitCode === 0, `expected exitCode=0 but got ${JSON.stringify(latest?.payload)}`);
}

const CASES = {
  "approval-request-zh-cn": runApprovalRequestZhCnV2,
  "budget-exhausted": runBudgetExhausted,
  "invalid-red-allows-write": runInvalidRedAllowsWrite,
  "legacy-flat-session-only": runLegacyFlatSessionOnly,
  "missing-journal-event-after-write": runMissingJournalEventAfterWrite,
  "prebash-approval-gate": runPrebashApprovalGate,
  "pretool-bash-pre-destructive-checkpoint": runPretoolBashCreatesPreDestructiveCheckpoint,
  "pretool-write-pre-destructive-checkpoint": runPretoolWriteCreatesPreDestructiveCheckpoint,
  "postwrite-journal-append": runPostwriteJournalAppend,
  "prewrite-red-gate": runPrewriteRedGate,
  "red-evidence-capture": runRedEvidenceCapture,
  "green-evidence-capture": runGreenEvidenceCapture,
  "shared-runtime-client": runSharedRuntimeClient,
  "hook-client-envelope": runSharedRuntimeClient,
  "stop-manual-restore-requires-human": runStopManualRestoreRequiresHuman,
  "stop-requires-resume-frontier": runStopRequiresResumeFrontier,
  "stop-without-resume-frontier": runStopWithoutResumeFrontier,
  "verify-evidence-capture": runVerifyEvidenceCapture,
};

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  const selected = parsed.caseName ? [[parsed.caseName, CASES[parsed.caseName]]] : Object.entries(CASES);
  if (selected.some(([, runner]) => typeof runner !== "function")) {
    console.error(`UNKNOWN_CASE ${parsed.caseName}`);
    process.exit(1);
  }

  let failed = false;
  for (const [caseName, runner] of selected) {
    try {
      runner();
      console.log(`CASE_PASS ${caseName}`);
      if (caseName === "legacy-flat-session-only") {
        console.log("HOOK_LEGACY_COMPAT_PASS");
      }
      if (caseName === "shared-runtime-client") {
        console.log("HOOK_RUNTIME_CLIENT_PASS");
      }
      if (["prewrite-red-gate", "approval-request-zh-cn"].includes(caseName)) {
        console.log("PREWRITE_V4_PASS");
      }
      if (["budget-exhausted", "stop-requires-resume-frontier"].includes(caseName)) {
        console.log("PREBASH_STOP_V4_PASS");
      }
      if (["postwrite-journal-append", "verify-evidence-capture"].includes(caseName)) {
        console.log("POSTHOOK_V4_PASS");
      }
      if (["prebash-approval-gate"].includes(caseName)) {
        console.log("PREBASH_APPROVAL_GATE_PASS");
      }
      if (["red-evidence-capture", "green-evidence-capture"].includes(caseName)) {
        console.log("POSTBASH_TDD_EVIDENCE_PASS");
      }
    } catch (error) {
      failed = true;
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
    }
  }

  if (failed) {
    process.exit(1);
  }
  console.log("HOOKS_V4_FIXTURES_PASS");
  console.log("HOOKS_SYSTEMIZATION_PASS");
}

main();
