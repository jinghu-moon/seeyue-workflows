"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

function deepMerge(baseValue, overrideValue) {
  if (overrideValue === undefined) {
    return structuredClone(baseValue);
  }
  if (
    baseValue &&
    overrideValue &&
    typeof baseValue === "object" &&
    typeof overrideValue === "object" &&
    !Array.isArray(baseValue) &&
    !Array.isArray(overrideValue)
  ) {
    const merged = { ...structuredClone(baseValue) };
    for (const [key, value] of Object.entries(overrideValue)) {
      merged[key] = key in merged ? deepMerge(merged[key], value) : structuredClone(value);
    }
    return merged;
  }
  return structuredClone(overrideValue);
}

function makeBaseSession() {
  return {
    schema: 4,
    run_id: "wf-20260308-101",
    engine: { kind: "codex", adapter_version: 1 },
    task: { id: "task-p2", title: "Runtime router and policy", mode: "feature" },
    phase: { current: "P2", status: "in_progress" },
    node: { active_id: "P2-N1", state: "red_pending", owner_persona: "author" },
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
      created_at: "2026-03-08T12:00:00Z",
      updated_at: "2026-03-08T12:00:00Z",
    },
  };
}

function makeBaseNode(nodeId, phaseId, options = {}) {
  return {
    id: nodeId,
    phase_id: phaseId,
    title: options.title || nodeId,
    target: options.target || `scripts/${nodeId}.cjs`,
    action: options.action || `Implement ${nodeId}`,
    why: options.why || `Why ${nodeId}`,
    depends_on: options.depends_on || [],
    verify: options.verify || { cmd: `node verify ${nodeId}`, pass_signal: `${nodeId}_PASS` },
    risk_level: options.risk_level || "medium",
    tdd_required: options.tdd_required ?? true,
    status: options.status || "pending",
    tdd_state: options.tdd_state || "idle",
    owner_persona: options.owner_persona || "author",
    review_state: options.review_state || { spec_review: "pending", quality_review: "pending" },
    evidence_refs: options.evidence_refs || [],
    output_refs: options.output_refs || [],
    approval_ref: options.approval_ref ?? null,
    capability: options.capability || "code_edit",
    priority: options.priority || "medium",
    parallel_group: options.parallel_group ?? null,
    condition: options.condition ?? null,
    retry_policy: options.retry_policy ?? null,
    timeout_policy: options.timeout_policy ?? null,
    test_contract:
      options.test_contract === undefined
        ? {
            layer: "unit",
            coverage_mode: "full",
            coverage_profile: "standard",
            mock_policy: "boundary_only",
            acceptance_criteria_refs: ["AC-1"],
            red_cmd: `node red ${nodeId}`,
            green_cmd: `node green ${nodeId}`,
            red_expectation: {
              allowed_failure_kinds: ["assertion_failure", "contract_mismatch", "behavior_result_mismatch"],
              rejected_failure_kinds: ["syntax_error", "environment_error", "permission_error"],
              allowed_exit_codes: [1],
              stderr_pattern: null,
              error_type: null,
            },
            behavior_gate: {
              ac_traceability_required: true,
              boundary_conditions_required: true,
            },
          }
        : options.test_contract,
  };
}

function makeBaseTaskGraph() {
  return {
    schema: 4,
    graph_id: "graph-p2",
    phases: [
      {
        id: "P1",
        title: "Foundation",
        status: "completed",
        depends_on: [],
        entry_condition: ["runtime foundation available"],
        exit_gate: { cmd: "node tests/runtime/run-runtime-store.cjs", pass_signal: "RUNTIME_STORE_PASS", coverage_min: "80%" },
        rollback_boundary: { revert_nodes: ["P1-N1"], restore_point: "foundation stable" },
      },
      {
        id: "P2",
        title: "Router And Policy",
        status: "in_progress",
        depends_on: ["P1"],
        entry_condition: ["P1 completed"],
        exit_gate: { cmd: "node tests/runtime/run-engine-kernel.cjs", pass_signal: "ENGINE_KERNEL_PASS", coverage_min: "90%" },
        rollback_boundary: { revert_nodes: ["P2-N1", "P2-N2", "P2-N3", "P2-N4"], restore_point: "P1 stable" },
      },
      {
        id: "P3",
        title: "Hooks",
        status: "pending",
        depends_on: ["P2"],
        entry_condition: ["P2 completed"],
        exit_gate: { cmd: "npm run test:hooks:smoke", pass_signal: "HOOKS_SMOKE_PASS", coverage_min: "90%" },
        rollback_boundary: { revert_nodes: ["P3-N1"], restore_point: "P2 stable" },
      },
    ],
    nodes: [
      makeBaseNode("P1-N1", "P1", {
        status: "completed",
        tdd_state: "verified",
        tdd_required: true,
        review_state: { spec_review: "pass", quality_review: "pass" },
        evidence_refs: ["foundation-evidence"],
        output_refs: ["foundation-output"],
      }),
      makeBaseNode("P2-N1", "P2", {
        status: "ready",
        tdd_state: "red_pending",
        risk_level: "high",
        priority: "medium",
      }),
      makeBaseNode("P2-N2", "P2", {
        status: "ready",
        tdd_required: false,
        tdd_state: "not_applicable",
        priority: "high",
      }),
      makeBaseNode("P2-N3", "P2", {
        status: "pending",
        depends_on: ["P2-N1"],
        priority: "low",
      }),
    ],
  };
}

function makeBaseSprintStatus() {
  return {
    schema: 4,
    active_phase: "P2",
    node_summary: [
      { id: "P2-N1", status: "ready", tdd_state: "red_pending" },
      { id: "P2-N2", status: "ready", tdd_state: "not_applicable" },
      { id: "P2-N3", status: "pending", tdd_state: "idle" },
    ],
    recommended_next: [],
  };
}

function applyOverridesById(items, overrides) {
  if (!overrides || typeof overrides !== "object") {
    return items;
  }
  return items.map((item) => (overrides[item.id] ? deepMerge(item, overrides[item.id]) : item));
}

function buildFixtureState(fixture = {}) {
  const session = deepMerge(makeBaseSession(), fixture.session || {});
  const taskGraph = makeBaseTaskGraph();
  taskGraph.phases = applyOverridesById(taskGraph.phases, fixture.phases);
  taskGraph.nodes = applyOverridesById(taskGraph.nodes, fixture.nodes);
  const sprintStatus = deepMerge(makeBaseSprintStatus(), fixture.sprint_status || {});
  return {
    session,
    taskGraph,
    sprintStatus,
    journal: structuredClone(fixture.journal || []),
    now: fixture.now || null,
    actionContext: structuredClone(fixture.actionContext || {}),
    validatorVerdict: deepMerge({ valid: true, issues: [] }, fixture.validatorVerdict || {}),
    policyVerdict: deepMerge(
      {
        route_effect: "allow",
        primary_reason: null,
        approval: { required: false, resolved: true, notify_only: false },
        retry: { allowed: false },
        timeout: { triggered: false },
        completion: { node_complete_ready: false, phase_complete_ready: false },
      },
      fixture.policyVerdict || {},
    ),
    expected: structuredClone(fixture.expected || {}),
  };
}

function loadFixtureFile(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

function loadFixtureMap(fixturesDir) {
  const entries = fs
    .readdirSync(fixturesDir)
    .filter((fileName) => fileName.endsWith(".json"))
    .sort();
  const map = new Map();
  for (const fileName of entries) {
    const caseName = fileName.replace(/\.json$/, "");
    map.set(caseName, loadFixtureFile(path.join(fixturesDir, fileName)));
  }
  return map;
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertSubset(actual, expected, prefix = "result") {
  if (Array.isArray(expected)) {
    assert(Array.isArray(actual), `${prefix} should be an array.`);
    assert(actual.length >= expected.length, `${prefix} should contain at least ${expected.length} items.`);
    for (let index = 0; index < expected.length; index += 1) {
      assertSubset(actual[index], expected[index], `${prefix}[${index}]`);
    }
    return;
  }
  if (expected && typeof expected === "object") {
    assert(actual && typeof actual === "object", `${prefix} should be an object.`);
    for (const [key, value] of Object.entries(expected)) {
      assertSubset(actual[key], value, `${prefix}.${key}`);
    }
    return;
  }
  assert(actual === expected, `${prefix} expected ${JSON.stringify(expected)} but got ${JSON.stringify(actual)}.`);
}

function makeTempRoot(prefix) {
  return fs.mkdtempSync(path.join(os.tmpdir(), prefix));
}

module.exports = {
  assertSubset,
  buildFixtureState,
  deepMerge,
  loadFixtureMap,
  makeBaseSession,
  makeBaseSprintStatus,
  makeBaseTaskGraph,
  makeTempRoot,
};
