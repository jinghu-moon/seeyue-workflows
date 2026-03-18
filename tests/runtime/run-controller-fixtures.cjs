"use strict";

const fs = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const {
  buildFixtureState,
  assertSubset,
  copyRuntimeFixtureFiles,
  makeTempRoot,
} = require("./runtime-fixture-lib.cjs");
const { buildReviewHandoffCapsule, createCapsule } = require("../../scripts/runtime/context-manager.cjs");
const {
  appendJournalEvents,
  readJournalEvents,
  readSession,
  readSprintStatus,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("../../scripts/runtime/store.cjs");

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  if (Array.isArray(state.journal) && state.journal.length > 0) {
    appendJournalEvents(rootDir, state.journal);
  }
  return state;
}

function runController(rootDir, mode, extraArgs = []) {
  const controllerPath = path.join(rootDir, "scripts", "runtime", "controller.cjs");
  const args = [controllerPath, "--root", rootDir];
  if (mode) {
    args.push("--mode", mode);
  }
  args.push(...extraArgs, "--json");
  return spawnSync(process.execPath, args, {
    cwd: rootDir,
    encoding: "utf8",
  });
}

function parseJsonOutput(result) {
  if (result.status !== 0) {
    throw new Error(`controller exited with ${result.status}: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout || "{}");
}

function writeReadyReport(rootDir) {
  const analysisDir = path.join(rootDir, ".ai", "analysis");
  fs.mkdirSync(analysisDir, { recursive: true });
  fs.writeFileSync(path.join(analysisDir, "ai.report.json"), JSON.stringify({
    overall: "READY",
    verification: {
      tests: [{ command: "npm run test:runtime:p2", exit_code: 0, key_signal: "ENGINE_KERNEL_PASS" }],
    },
  }, null, 2));
}

function seedReviewHandoff(rootDir, options = {}) {
  const sourcePersona = options.sourcePersona || "author";
  const sourceCapsule = createCapsule(rootDir, {
    persona: sourcePersona,
    inputSummary: `${sourcePersona} implementation summary`,
    outputSummary: `${sourcePersona} handoff ready`,
    verdict: "pending",
  });
  return buildReviewHandoffCapsule(rootDir, { sourceCapsule, targetPersona: options.targetPersona });
}

const cases = {
  "run-starts-route-cycle": () => {
    const rootDir = makeTempRoot("controller-run-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "run"));
    assertSubset(result, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_node: "P2-N2",
        recommended_next: [{ type: "start_node", target: "P2-N2" }],
      },
    });

    const session = readSession(rootDir);
    const sprintStatus = readSprintStatus(rootDir);
    const taskGraph = readTaskGraph(rootDir);
    const journal = readJournalEvents(rootDir);

    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N2", owner_persona: "author" },
    });
    assertSubset(sprintStatus, {
      active_phase: "P2",
      recommended_next: [{ type: "start_node", target: "P2-N2" }],
    });
    const startedNode = taskGraph.nodes.find((item) => item.id === "P2-N2");
    if (!startedNode || startedNode.status !== "in_progress") {
      throw new Error("expected P2-N2 to become in_progress");
    }
    if (!journal.some((item) => item.event === "session_started")) {
      throw new Error("expected session_started event");
    }
    if (!journal.some((item) => item.event === "node_started")) {
      throw new Error("expected node_started event");
    }
  },
  "run-emits-node-bypassed-for-conditional-ready-node": () => {
    const rootDir = makeTempRoot("controller-run-bypass-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
      nodes: {
        "P2-N1": {
          status: "ready",
          tdd_required: false,
          tdd_state: "not_applicable",
          condition: { mode: "state_expression", expression: "session.phase.current == P1" },
        },
        "P2-N2": {
          status: "ready",
          tdd_required: false,
          tdd_state: "not_applicable",
          priority: "high",
        },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "run"));
    assertSubset(result, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_node: "P2-N2",
        recommended_next: [{ type: "start_node", target: "P2-N2" }],
      },
    });

    const taskGraph = readTaskGraph(rootDir);
    const journal = readJournalEvents(rootDir);
    const bypassedNode = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!bypassedNode || bypassedNode.status !== "completed") {
      throw new Error("expected P2-N1 to be marked completed after bypass");
    }
    if (!journal.some((item) => item.event === "node_bypassed" && item.node_id === "P2-N1")) {
      throw new Error("expected node_bypassed event for P2-N1");
    }
    const startedNode = taskGraph.nodes.find((item) => item.id === "P2-N2");
    if (!startedNode || startedNode.status !== "in_progress") {
      throw new Error("expected P2-N2 to become in_progress");
    }
  },
  "run-auto-loop-enters-next-phase-and-starts-first-node": () => {
    const rootDir = makeTempRoot("controller-run-auto-loop-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "completed" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
      phases: {
        "P2": { status: "completed" },
        "P3": { status: "pending" },
      },
      nodes: {
        "P2-N1": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
      sprint_status: {
        active_phase: "P2",
      },
    });
    const taskGraph = readTaskGraph(rootDir);
    taskGraph.nodes.push({
      id: "P3-N1",
      phase_id: "P3",
      title: "P3-N1",
      target: "scripts/P3-N1.cjs",
      action: "Implement P3-N1",
      why: "Enter P3 execution",
      depends_on: [],
      verify: { cmd: "node verify P3-N1", pass_signal: "P3-N1_PASS" },
      risk_level: "medium",
      tdd_required: false,
      status: "ready",
      tdd_state: "not_applicable",
      owner_persona: "author",
      review_state: { spec_review: "pending", quality_review: "pending" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: null,
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: null,
    });
    writeTaskGraph(rootDir, taskGraph);

    const result = parseJsonOutput(runController(rootDir, "run", ["--auto-loop"]));
    assertSubset(result, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_phase: "P3",
        active_node: "P3-N1",
        recommended_next: [{ type: "start_node", target: "P3-N1" }],
      },
      loop_summary: {
        enabled: true,
        hops_executed: 1,
        stop_reason: "author_frontier_reached",
        frontier_kind: "author",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P3", status: "in_progress" },
      node: { active_id: "P3-N1", owner_persona: "author" },
    });
  },
  "run-auto-loop-stops-at-max-hops": () => {
    const rootDir = makeTempRoot("controller-run-max-hops-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "completed" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
      phases: {
        "P2": { status: "completed" },
        "P3": { status: "pending" },
      },
      nodes: {
        "P2-N1": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "run", ["--auto-loop", "--max-hops", "0"]));
    assertSubset(result, {
      mode: "run",
      decision: {
        route_verdict: "advance",
        active_phase: "P3",
        active_node: "none",
        recommended_next: [{ type: "enter_phase", target: "P3" }],
      },
      loop_summary: {
        enabled: true,
        hops_executed: 0,
        stop_reason: "max_hops_reached",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P3", status: "in_progress" },
      node: { active_id: "none", owner_persona: "planner" },
    });
  },
  "run-auto-loop-max-hops-multi-phase-advance": () => {
    const rootDir = makeTempRoot("controller-run-max-hops-multi-phase-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P1", status: "completed" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
      phases: {
        "P1": { status: "completed" },
        "P2": { status: "pending" },
        "P3": { status: "pending" },
      },
      nodes: {
        "P2-N1": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
      sprint_status: {
        active_phase: "P1",
      },
    });
    const taskGraph = readTaskGraph(rootDir);
    taskGraph.phases.push({
      id: "P4",
      title: "P4",
      status: "pending",
      depends_on: ["P3"],
      entry_condition: ["P3 completed"],
      exit_gate: { cmd: "node tests/runtime/run-controller-fixtures.cjs", pass_signal: "CONTROLLER_FIXTURES_PASS", coverage_min: "80%" },
      rollback_boundary: { revert_nodes: ["P4-N1"], restore_point: "P3 stable" },
    });
    taskGraph.nodes.push({
      id: "P3-N1",
      phase_id: "P3",
      title: "P3-N1",
      target: "scripts/P3-N1.cjs",
      action: "Implement P3-N1",
      why: "P3 is pre-completed for loop stress",
      depends_on: [],
      verify: { cmd: "node verify P3-N1", pass_signal: "P3-N1_PASS" },
      risk_level: "medium",
      tdd_required: false,
      status: "completed",
      tdd_state: "not_applicable",
      owner_persona: "author",
      review_state: { spec_review: "pass", quality_review: "pass" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: null,
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: null,
    });
    taskGraph.nodes.push({
      id: "P4-N1",
      phase_id: "P4",
      title: "P4-N1",
      target: "scripts/P4-N1.cjs",
      action: "Implement P4-N1",
      why: "Provide a ready node after multi-phase advance",
      depends_on: [],
      verify: { cmd: "node verify P4-N1", pass_signal: "P4-N1_PASS" },
      risk_level: "medium",
      tdd_required: false,
      status: "ready",
      tdd_state: "not_applicable",
      owner_persona: "author",
      review_state: { spec_review: "pending", quality_review: "pending" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: null,
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: null,
    });
    writeTaskGraph(rootDir, taskGraph);

    const result = parseJsonOutput(runController(rootDir, "run", ["--auto-loop", "--max-hops", "2"]));
    assertSubset(result, {
      mode: "run",
      loop_summary: {
        enabled: true,
        hops_executed: 2,
        stop_reason: "max_hops_reached",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P4", status: "in_progress" },
      node: { active_id: "none", owner_persona: "planner" },
    });
  },
  "run-auto-loop-stops-at-max-hops-long": () => {
    const rootDir = makeTempRoot("controller-run-long-hops-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P1", status: "completed" },
        node: { active_id: "none", state: "idle", owner_persona: "planner" },
      },
      phases: {
        "P1": { status: "completed" },
        "P2": { status: "pending" },
        "P3": { status: "pending" },
      },
      nodes: {
        "P2-N1": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
      sprint_status: {
        active_phase: "P1",
      },
    });

    const makeCompletedNode = (id, phaseId) => ({
      id,
      phase_id: phaseId,
      title: id,
      target: `scripts/${id}.cjs`,
      action: `Implement ${id}`,
      why: "Pre-completed for long loop coverage",
      depends_on: [],
      verify: { cmd: `node verify ${id}`, pass_signal: `${id}_PASS` },
      risk_level: "medium",
      tdd_required: false,
      status: "completed",
      tdd_state: "not_applicable",
      owner_persona: "author",
      review_state: { spec_review: "pass", quality_review: "pass" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: null,
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: null,
    });

    const taskGraph = readTaskGraph(rootDir);
    taskGraph.phases = taskGraph.phases.map((phase) => {
      if (phase.id === "P1") {
        return { ...phase, status: "completed", depends_on: [] };
      }
      if (phase.id === "P2") {
        return { ...phase, status: "pending", depends_on: ["P1"] };
      }
      if (phase.id === "P3") {
        return { ...phase, status: "pending", depends_on: ["P2"] };
      }
      return phase;
    });
    taskGraph.phases.push(
      {
        id: "P4",
        title: "P4",
        status: "pending",
        depends_on: ["P3"],
        entry_condition: ["P3 completed"],
        exit_gate: { cmd: "node tests/runtime/run-controller-fixtures.cjs", pass_signal: "CONTROLLER_FIXTURES_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P4-N1"], restore_point: "P3 stable" },
      },
      {
        id: "P5",
        title: "P5",
        status: "pending",
        depends_on: ["P4"],
        entry_condition: ["P4 completed"],
        exit_gate: { cmd: "node tests/runtime/run-controller-fixtures.cjs", pass_signal: "CONTROLLER_FIXTURES_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P5-N1"], restore_point: "P4 stable" },
      },
      {
        id: "P6",
        title: "P6",
        status: "pending",
        depends_on: ["P5"],
        entry_condition: ["P5 completed"],
        exit_gate: { cmd: "node tests/runtime/run-controller-fixtures.cjs", pass_signal: "CONTROLLER_FIXTURES_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P6-N1"], restore_point: "P5 stable" },
      },
      {
        id: "P7",
        title: "P7",
        status: "pending",
        depends_on: ["P6"],
        entry_condition: ["P6 completed"],
        exit_gate: { cmd: "node tests/runtime/run-controller-fixtures.cjs", pass_signal: "CONTROLLER_FIXTURES_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P7-N1"], restore_point: "P6 stable" },
      },
    );
    taskGraph.nodes.push(
      makeCompletedNode("P3-N1", "P3"),
      makeCompletedNode("P4-N1", "P4"),
      makeCompletedNode("P5-N1", "P5"),
      makeCompletedNode("P6-N1", "P6"),
    );
    writeTaskGraph(rootDir, taskGraph);

    const result = parseJsonOutput(runController(rootDir, "run", ["--auto-loop", "--max-hops", "5"]));
    assertSubset(result, {
      mode: "run",
      loop_summary: {
        enabled: true,
        hops_executed: 5,
        stop_reason: "max_hops_reached",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P7", status: "in_progress" },
      node: { active_id: "none", owner_persona: "planner" },
    });
  },
  "run-auto-loop-stops-at-spec-review-frontier": () => {
    const rootDir = makeTempRoot("controller-run-spec-review-frontier-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "green_verified", owner_persona: "author" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: true,
          tdd_state: "green_verified",
          owner_persona: "author",
          review_state: { spec_review: "pending", quality_review: "pending" },
        },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "run", ["--auto-loop"]));
    assertSubset(result, {
      mode: "run",
      decision: {
        route_verdict: "handoff",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "spec_reviewer",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
      loop_summary: {
        enabled: true,
        stop_reason: "review_frontier_reached",
        frontier_kind: "review",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "review" },
      node: { active_id: "P2-N1", owner_persona: "spec_reviewer" },
    });
  },
  "run-auto-loop-stops-at-quality-review-frontier": () => {
    const rootDir = makeTempRoot("controller-run-quality-review-frontier-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "green_verified", owner_persona: "spec_reviewer" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: true,
          tdd_state: "green_verified",
          owner_persona: "spec_reviewer",
          review_state: { spec_review: "pass", quality_review: "pending" },
        },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "run", ["--auto-loop"]));
    assertSubset(result, {
      mode: "run",
      decision: {
        route_verdict: "handoff",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "quality_reviewer",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
      loop_summary: {
        enabled: true,
        stop_reason: "review_frontier_reached",
        frontier_kind: "review",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "review" },
      node: { active_id: "P2-N1", owner_persona: "quality_reviewer" },
    });
  },
  "resume-restore-pending-route": () => {
    const rootDir = makeTempRoot("controller-resume-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        node: { active_id: "P2-N1", state: "red_pending", owner_persona: "author" },
        recovery: { restore_pending: true, restore_reason: "missing_terminal_event" },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "resume"));
    assertSubset(result, {
      mode: "resume",
      decision: {
        route_verdict: "resume",
        block_reason: "restore_pending",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
    });

    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "session_resumed")) {
      throw new Error("expected session_resumed event");
    }
  },
  "resume-manual-restore-blocks-for-human": () => {
    const rootDir = makeTempRoot("controller-resume-manual-restore-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "author" },
        recovery: { restore_pending: true, restore_reason: "missing_tool_call_metadata" },
      },
    });

    const result = parseJsonOutput(runController(rootDir, "resume"));
    assertSubset(result, {
      mode: "resume",
      decision: {
        route_verdict: "block",
        block_reason: "restore_requires_human",
        recommended_next: [{ type: "human_intervention", target: "P2-N1" }],
      },
    });

    const journal = readJournalEvents(rootDir);
    if (journal.some((item) => item.event === "session_resumed")) {
      throw new Error("manual restore should not emit session_resumed");
    }
  },
  "resume-auto-detects-interrupted-run": () => {
    const rootDir = makeTempRoot("controller-resume-auto-recovery-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "author" },
      },
      nodes: {
        "P2-N1": {
          status: "in_progress",
          tdd_state: "green_pending",
        },
      },
      journal: [
        {
          ts: new Date(Date.now() - 60_000).toISOString(),
          run_id: "wf-20260308-101",
          event: "node_started",
          phase: "P2",
          node_id: "P2-N1",
          actor: "runtime",
          payload: { source: "transition_applier" },
          trace_id: "trace-auto-recovery-1",
        },
      ],
    });

    const result = parseJsonOutput(runController(rootDir, "resume"));
    assertSubset(result, {
      mode: "resume",
      recovery: {
        recovery_required: true,
      },
      decision: {
        route_verdict: "resume",
        block_reason: "restore_pending",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
    });

    const session = readSession(rootDir);
    const journal = readJournalEvents(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N1", owner_persona: "author" },
    });
    if (!journal.some((item) => item.event === "validation_failed" && item.payload?.reason === "missing_terminal_event")) {
      throw new Error("expected validation_failed event for missing terminal event");
    }
    if (!journal.some((item) => item.event === "session_resumed" && item.node_id === "P2-N1")) {
      throw new Error("expected session_resumed event after auto recovery detection");
    }
  },
  "resume-derives-retry-from-journal": () => {
    const rootDir = makeTempRoot("controller-retry-journal-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "human" },
      },
      nodes: {
        "P2-N1": {
          status: "failed",
          tdd_state: "green_pending",
          retry_policy: {
            max_attempts: 3,
            retry_on: ["contract_mismatch", "timeout"],
            backoff_mode: "exponential",
          },
        },
      },
      journal: [
        {
          ts: "2026-03-08T12:05:00Z",
          run_id: "wf-20260308-101",
          event: "node_failed",
          phase: "P2",
          node_id: "P2-N1",
          actor: "runtime",
          payload: {
            source: "transition_applier",
            transition_context: {
              failure_kind: "contract_mismatch",
            },
          },
          trace_id: "trace-retry-journal-1",
        },
      ],
    });

    const result = parseJsonOutput(runController(rootDir, "resume"));
    assertSubset(result, {
      mode: "resume",
      decision: {
        route_verdict: "resume",
        active_node: "P2-N1",
        policy_verdict: {
          retry: { allowed: true, next_attempt: 2 },
        },
        recommended_next: [{ type: "retry_node", target: "P2-N1", params: { next_attempt: 2 } }],
      },
    });

    const session = readSession(rootDir);
    const taskGraph = readTaskGraph(rootDir);
    const journal = readJournalEvents(rootDir);

    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N1", owner_persona: "author" },
    });
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!node || node.status !== "in_progress") {
      throw new Error("expected P2-N1 to resume in_progress after retry routing");
    }
    if (!journal.some((item) => item.event === "node_started" && item.node_id === "P2-N1")) {
      throw new Error("expected node_started event after retry routing");
    }
  },
  "resume-holds-when-retry-backoff-pending": () => {
    const rootDir = makeTempRoot("controller-retry-backoff-");
    copyRuntimeFixtureFiles(rootDir);
    const failureTs = new Date(Date.now() - 60_000).toISOString();
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "failed", owner_persona: "human" },
      },
      nodes: {
        "P2-N1": {
          status: "failed",
          tdd_state: "green_pending",
          retry_policy: {
            max_attempts: 3,
            retry_on: ["contract_mismatch"],
            backoff_mode: "fixed",
            initial_delay_seconds: 600,
          },
        },
      },
      journal: [
        {
          ts: failureTs,
          run_id: "wf-20260308-101",
          event: "node_failed",
          phase: "P2",
          node_id: "P2-N1",
          actor: "runtime",
          payload: {
            source: "transition_applier",
            transition_context: {
              failure_kind: "contract_mismatch",
            },
          },
          trace_id: "trace-retry-backoff-1",
        },
      ],
    });

    const result = parseJsonOutput(runController(rootDir, "resume"));
    assertSubset(result, {
      mode: "resume",
      decision: {
        route_verdict: "hold",
        block_reason: "retry_backoff_pending",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
    });

    const session = readSession(rootDir);
    const taskGraph = readTaskGraph(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "blocked" },
      node: { active_id: "P2-N1", state: "failed", owner_persona: "author" },
    });
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    if (!node || node.status !== "failed") {
      throw new Error("expected failed node to remain failed while backoff is pending");
    }
  },
  "approval-approved-refreshes-next": () => {
    const rootDir = makeTempRoot("controller-approval-approved-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "none",
          last_approval_mode: "manual_required",
          active_request: {
            approval_id: "apr-ctrl-001",
            action: "write_file",
            target_ref: "P2-N1",
            risk_class: "critical",
            approval_mode: "manual_required",
            grant_scope: "once",
            status: "pending",
            requested_at: "2026-03-09T18:00:00Z",
          },
          grants: [],
        },
      },
      nodes: {
        "P2-N1": {
          status: "blocked",
          tdd_state: "green_pending",
          approval_ref: "apr-ctrl-001",
        },
      },
      sprint_status: {
        recommended_next: [{ type: "request_approval", target: "P2-N1" }],
      },
    });

    const result = parseJsonOutput(runController(rootDir, null, [
      "--approval-decision", "approved",
      "--approval-id", "apr-ctrl-001",
      "--approval-actor", "human",
      "--approval-reason", "looks safe",
    ]));

    assertSubset(result, {
      mode: "approval",
      approval: {
        decision: "approved",
        approval_id: "apr-ctrl-001",
        pending_cleared: true,
      },
      decision: {
        route_verdict: "resume",
        active_phase: "P2",
        active_node: "P2-N1",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
      loop_summary: {
        enabled: false,
        hops_executed: 0,
        stop_reason: "auto_loop_disabled",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N1", owner_persona: "author" },
      approvals: { pending: false, pending_count: 0, active_request: null },
    });
  },
  "approval-approved-auto-loop-enters-next-phase-and-starts-first-node": () => {
    const rootDir = makeTempRoot("controller-approval-auto-loop-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "none", state: "idle", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "none",
          last_approval_mode: "manual_required",
          active_request: {
            approval_id: "apr-phase-001",
            action: "phase_gate",
            target_ref: "P2",
            risk_class: "high",
            approval_mode: "manual_required",
            grant_scope: "once",
            status: "pending",
            requested_at: "2026-03-09T18:30:00Z",
          },
          grants: [],
        },
      },
      phases: {
        "P2": { status: "completed" },
        "P3": { status: "pending" },
      },
      nodes: {
        "P2-N1": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });
    const taskGraph = readTaskGraph(rootDir);
    taskGraph.nodes.push({
      id: "P3-N1",
      phase_id: "P3",
      title: "P3-N1",
      target: "scripts/P3-N1.cjs",
      action: "Implement P3-N1",
      why: "Start P3 after approval",
      depends_on: [],
      verify: { cmd: "node verify P3-N1", pass_signal: "P3-N1_PASS" },
      risk_level: "medium",
      tdd_required: false,
      status: "ready",
      tdd_state: "not_applicable",
      owner_persona: "author",
      review_state: { spec_review: "pending", quality_review: "pending" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: null,
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: null,
    });
    writeTaskGraph(rootDir, taskGraph);

    const result = parseJsonOutput(runController(rootDir, null, [
      "--approval-decision", "approved",
      "--approval-id", "apr-phase-001",
      "--approval-actor", "human",
      "--approval-reason", "phase gate approved",
      "--auto-loop",
    ]));

    assertSubset(result, {
      mode: "approval",
      approval: {
        decision: "approved",
        approval_id: "apr-phase-001",
      },
      decision: {
        route_verdict: "advance",
        active_phase: "P3",
        active_node: "P3-N1",
        recommended_next: [{ type: "start_node", target: "P3-N1" }],
      },
      loop_summary: {
        enabled: true,
        hops_executed: 2,
        stop_reason: "author_frontier_reached",
        frontier_kind: "author",
      },
    });

    if (!Array.isArray(result.decision_chain) || result.decision_chain.length < 3) {
      throw new Error("expected approval decision chain to include approval snapshot and runtime follow-up decisions");
    }

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P3", status: "in_progress" },
      node: { active_id: "P3-N1", owner_persona: "author" },
      approvals: { pending: false, pending_count: 0, active_request: null },
    });
  },
  "approval-rejected-hands-back-to-human": () => {
    const rootDir = makeTempRoot("controller-approval-rejected-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "blocked" },
        node: { active_id: "P2-N1", state: "green_pending", owner_persona: "human" },
        approvals: {
          pending: true,
          pending_count: 1,
          last_grant_scope: "none",
          last_approval_mode: "never_auto",
          active_request: {
            approval_id: "apr-ctrl-002",
            action: "edit_file",
            target_ref: "P2-N1",
            risk_class: "high",
            approval_mode: "never_auto",
            grant_scope: "once",
            status: "pending",
            requested_at: "2026-03-09T18:00:00Z",
          },
          grants: [],
        },
      },
      nodes: {
        "P2-N1": {
          status: "blocked",
          approval_ref: "apr-ctrl-002",
        },
      },
    });

    const result = parseJsonOutput(runController(rootDir, null, [
      "--approval-decision", "rejected",
      "--approval-id", "apr-ctrl-002",
      "--approval-actor", "human",
      "--approval-reason", "too risky",
    ]));

    assertSubset(result, {
      mode: "approval",
      approval: {
        decision: "rejected",
        approval_id: "apr-ctrl-002",
        pending_cleared: true,
      },
      decision: {
        route_verdict: "hold",
        active_phase: "P2",
        active_node: "P2-N1",
        recommended_next: [{ type: "human_intervention", target: "P2" }],
      },
      loop_summary: {
        enabled: false,
        hops_executed: 0,
        stop_reason: "auto_loop_disabled",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "blocked" },
      node: { active_id: "P2-N1", owner_persona: "human" },
      approvals: { pending: false, pending_count: 0, active_request: null },
    });
  },
  "review-spec-pass-auto-loop-handoffs-to-quality-review-frontier": () => {
    const rootDir = makeTempRoot("controller-review-spec-pass-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "spec_reviewer" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: false,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pending", quality_review: "pending" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
      },
    });
    writeReadyReport(rootDir);
    seedReviewHandoff(rootDir, { sourcePersona: "author", targetPersona: "spec_reviewer" });

    const result = parseJsonOutput(runController(rootDir, null, [
      "--review-decision", "pass",
      "--review-persona", "spec_reviewer",
      "--review-reason", "spec accepted",
      "--auto-loop",
    ]));

    assertSubset(result, {
      mode: "review",
      review: {
        decision: "pass",
        reviewer_persona: "spec_reviewer",
      },
      decision: {
        route_verdict: "handoff",
        active_phase: "P2",
        active_node: "P2-N1",
        next_persona: "quality_reviewer",
        recommended_next: [{ type: "resume_node", target: "P2-N1" }],
      },
      loop_summary: {
        enabled: true,
        stop_reason: "review_frontier_reached",
        frontier_kind: "review",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "review" },
      node: { active_id: "P2-N1", owner_persona: "quality_reviewer" },
    });
  },
  "review-quality-pass-auto-loop-starts-next-ready-node": () => {
    const rootDir = makeTempRoot("controller-review-quality-pass-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: false,
          tdd_state: "not_applicable",
          test_contract: null,
          review_state: { spec_review: "pass", quality_review: "pending" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
        "P2-N2": {
          status: "ready",
          tdd_required: false,
          tdd_state: "not_applicable",
          test_contract: null,
          review_state: { spec_review: "pending", quality_review: "pending" },
        },
      },
    });
    writeReadyReport(rootDir);
    seedReviewHandoff(rootDir, { sourcePersona: "spec_reviewer", targetPersona: "quality_reviewer" });

    const result = parseJsonOutput(runController(rootDir, null, [
      "--review-decision", "pass",
      "--review-persona", "quality_reviewer",
      "--review-reason", "quality accepted",
      "--auto-loop",
    ]));

    assertSubset(result, {
      mode: "review",
      review: {
        decision: "pass",
        reviewer_persona: "quality_reviewer",
      },
      decision: {
        route_verdict: "advance",
        active_phase: "P2",
        active_node: "P2-N2",
        next_persona: "author",
        recommended_next: [{ type: "start_node", target: "P2-N2" }],
      },
      loop_summary: {
        enabled: true,
        stop_reason: "author_frontier_reached",
        frontier_kind: "author",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N2", owner_persona: "author" },
      loop_budget: { consumed_nodes: 1 },
    });
  },
  "verify-review-phase-reads-ready-report": () => {
    const rootDir = makeTempRoot("controller-verify-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N2": {
          status: "review",
          tdd_required: false,
          tdd_state: "verified",
          test_contract: null,
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: ["verify-staging.json"],
        },
      },
      policyVerdict: {
        completion: { node_complete_ready: true, phase_complete_ready: true },
      },
    });
    fs.mkdirSync(path.join(rootDir, ".ai", "analysis"), { recursive: true });
    fs.writeFileSync(
      path.join(rootDir, ".ai", "analysis", "ai.report.json"),
      JSON.stringify({
        overall: "READY",
        verification: {
          tests: [{ command: "npm run test:runtime:p2", exit_code: 0, key_signal: "ENGINE_KERNEL_PASS" }],
        },
      }, null, 2),
    );

    const result = parseJsonOutput(runController(rootDir, "verify"));
    assertSubset(result, {
      mode: "verify",
      decision: {
        policy_verdict: {
          completion: { node_complete_ready: true },
        },
      },
      verification: {
        phase_status: "review",
        report_overall: "READY",
        review_ready: true,
        report_ref: ".ai/analysis/ai.report.json",
      },
    });
  },
  "verify-builds-action-context-from-existing-report": () => {
    const rootDir = makeTempRoot("controller-verify-evidence-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N2", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N2": {
          status: "review",
          tdd_required: false,
          tdd_state: "verified",
          test_contract: null,
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
      },
    });
    writeReadyReport(rootDir);
    fs.writeFileSync(
      path.join(rootDir, ".ai", "analysis", "ai.report.json"),
      JSON.stringify({
        overall: "READY",
        verification: {
          tests: [{ command: "npm run test:runtime:p2", exit_code: 0, key_signal: "ENGINE_KERNEL_PASS" }],
        },
      }, null, 2),
    );

    const result = parseJsonOutput(runController(rootDir, "verify"));
    assertSubset(result, {
      mode: "verify",
      decision: {
        policy_verdict: {
          completion: { node_complete_ready: true },
        },
      },
      verification: {
        report_exists: true,
        report_overall: "READY",
        review_ready: true,
      },
    });
  },
  "verify-auto-advance-starts-next-ready-node": () => {
    const rootDir = makeTempRoot("controller-verify-auto-start-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
        "P2-N2": {
          status: "ready",
          tdd_required: false,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pending", quality_review: "pending" },
        },
      },
    });
    writeReadyReport(rootDir);

    const result = parseJsonOutput(runController(rootDir, "verify", ["--auto-advance"]));
    assertSubset(result, {
      mode: "verify",
      decision: {
        route_verdict: "advance",
        active_node: "P2-N2",
        recommended_next: [{ type: "start_node", target: "P2-N2" }],
      },
      verification: {
        phase_status: "in_progress",
        active_node: "P2-N2",
        report_overall: "READY",
      },
      loop_summary: {
        enabled: true,
        stop_reason: "author_frontier_reached",
        frontier_kind: "author",
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "in_progress" },
      node: { active_id: "P2-N2", owner_persona: "author" },
      loop_budget: { consumed_nodes: 1 },
    });
  },
  "verify-auto-advance-enters-next-phase-and-starts-first-node": () => {
    const rootDir = makeTempRoot("controller-verify-auto-phase-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      phases: {
        "P3": { status: "pending" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: false,
          test_contract: null,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_required: true,
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });
    const taskGraph = readTaskGraph(rootDir);
    taskGraph.nodes.push({
      id: "P3-N1",
      phase_id: "P3",
      title: "P3-N1",
      target: "scripts/P3-N1.cjs",
      action: "Implement P3-N1",
      why: "Enter P3 execution",
      depends_on: [],
      verify: { cmd: "node verify P3-N1", pass_signal: "P3-N1_PASS" },
      risk_level: "medium",
      tdd_required: false,
      status: "ready",
      tdd_state: "not_applicable",
      owner_persona: "author",
      review_state: { spec_review: "pending", quality_review: "pending" },
      evidence_refs: [],
      output_refs: [],
      approval_ref: null,
      capability: "code_edit",
      priority: "high",
      parallel_group: null,
      condition: null,
      retry_policy: null,
      timeout_policy: null,
      test_contract: null,
    });
    writeTaskGraph(rootDir, taskGraph);
    writeReadyReport(rootDir);

    const result = parseJsonOutput(runController(rootDir, "verify", ["--auto-advance"]));
    assertSubset(result, {
      mode: "verify",
      decision: {
        route_verdict: "advance",
        active_phase: "P3",
        active_node: "P3-N1",
        recommended_next: [{ type: "start_node", target: "P3-N1" }],
      },
      verification: {
        phase_status: "in_progress",
        phase_current: "P3",
        active_node: "P3-N1",
      },
      loop_summary: {
        enabled: true,
        hops_executed: 1,
        stop_reason: "author_frontier_reached",
        frontier_kind: "author",
      },
    });

    if (!Array.isArray(result.decision_chain) || result.decision_chain.length < 2) {
      throw new Error("expected decision_chain to record enter_phase and start_node decisions");
    }
    assertSubset(result.decision_chain[0], {
      active_phase: "P3",
      recommended_next: [{ type: "enter_phase", target: "P3" }],
    });
    assertSubset(result.decision_chain[1], {
      active_phase: "P3",
      active_node: "P3-N1",
      recommended_next: [{ type: "start_node", target: "P3-N1" }],
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P3", status: "in_progress" },
      node: { active_id: "P3-N1", owner_persona: "author" },
      loop_budget: { consumed_nodes: 1 },
    });

    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "phase_entered" && item.phase === "P3")) {
      throw new Error("expected phase_entered event for P3");
    }
    if (!journal.some((item) => item.event === "node_started" && item.node_id === "P3-N1")) {
      throw new Error("expected node_started event for P3-N1");
    }
  },
  "verify-stop-gate-finalizes-session": () => {
    const rootDir = makeTempRoot("controller-verify-stop-gate-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "review" },
        node: { active_id: "P2-N1", state: "verified", owner_persona: "quality_reviewer" },
      },
      phases: {
        "P3": { status: "completed" },
      },
      nodes: {
        "P2-N1": {
          status: "review",
          tdd_required: false,
          test_contract: null,
          tdd_state: "green_verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
        "P2-N2": {
          status: "completed",
          tdd_required: false,
          tdd_state: "not_applicable",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
        "P2-N3": {
          status: "completed",
          tdd_state: "verified",
          review_state: { spec_review: "pass", quality_review: "pass" },
        },
      },
    });
    writeReadyReport(rootDir);

    const result = parseJsonOutput(runController(rootDir, "verify"));
    assertSubset(result, {
      mode: "verify",
      decision: {
        route_verdict: "hold",
        policy_verdict: {
          completion: { stop_gate_ready: true },
        },
        recommended_next: [{ type: "human_intervention", target: "session" }],
      },
      verification: {
        phase_status: "completed",
        active_node: "none",
        report_exists: true,
      },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      phase: { current: "P2", status: "completed" },
      node: { active_id: "none", state: "idle", owner_persona: "human" },
    });

    const journal = readJournalEvents(rootDir);
    if (!journal.some((item) => item.event === "session_stopped")) {
      throw new Error("expected session_stopped event");
    }
  },

  "interaction-blocks-loop": () => {
    // GREEN: session with active interaction → controller loop must stop with interaction_pending
    const rootDir = makeTempRoot("controller-interaction-blocks-");
    copyRuntimeFixtureFiles(rootDir);

    // Write session with active interaction via fixture helpers
    writeRuntimeState(rootDir, {
      session: {
        phase: { current: "P2", status: "in_progress" },
        node: { active_id: "P2-N1", state: "in_progress", owner_persona: "author" },
        interaction: {
          active_interaction_id: "ix-20260318-001",
          pending_count: 1,
          last_dispatched_at: null,
          blocking_kind: "approval",
          blocking_reason: "destructive_write_requires_approval",
        },
      },
    });

    const result = spawnSync(
      process.execPath,
      [path.join(rootDir, "scripts", "runtime", "controller.cjs"),
       "--root", rootDir, "--mode", "run", "--auto-loop", "--json"],
      { cwd: rootDir, encoding: "utf8" },
    );

    let output;
    try {
      output = JSON.parse(result.stdout || "{}");
    } catch {
      throw new Error(`controller output not JSON: ${result.stdout || result.stderr}`);
    }

    // With --auto-loop, loop_summary.stop_reason must be interaction_pending
    const stopReason = output?.loop_summary?.stop_reason;
    if (stopReason !== "interaction_pending") {
      throw new Error(`expected stop_reason=interaction_pending, got: ${stopReason}`);
    }
  },
};

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
    throw new Error(`Unknown case: ${parsed.caseName}`);
  }
  for (const [caseName, run] of selected) {
    try {
      run();
      console.log(`CASE_PASS ${caseName}`);
    } catch (error) {
      console.error(`CASE_FAIL ${caseName}`);
      console.error(error.stack || error.message);
      process.exit(1);
    }
  }
  if (!parsed.caseName) {
    console.log("CONTROLLER_FIXTURES_PASS");
  }
}

main();
