#!/usr/bin/env node
"use strict";

const path = require("node:path");

const { appendEvent } = require("./journal.cjs");
const { readSession, readTaskGraph, writeSession, writeTaskGraph } = require("./store.cjs");
const { loadWorkflowSpecs } = require("./workflow-specs.cjs");

function nowIso() {
  return new Date().toISOString();
}

function clone(value) {
  return value === undefined ? undefined : structuredClone(value);
}

function buildParallelGroupRepairs(taskGraph, specs) {
  const nodes = Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : [];
  const parallelNodesSupported = specs?.routerSpec?.node_routing?.parallel_nodes_supported;
  if (parallelNodesSupported !== false) {
    return [];
  }

  return nodes
    .filter((node) => node && node.parallel_group !== null && node.parallel_group !== undefined)
    .map((node) => ({
      repair_kind: "clear_parallel_group_v1",
      severity: "safe_auto_repair",
      target: `node:${node.id}`,
      path: `.ai/workflow/task-graph.yaml#nodes[id=${node.id}].parallel_group`,
      before: node.parallel_group,
      after: null,
      reason: "router.spec.yaml declares single-active-node execution in V1",
    }));
}

function inspectRuntimeStateRepair(rootDir, options = {}) {
  const rootPath = path.resolve(rootDir);
  const specs = loadWorkflowSpecs(options.specRootDir || rootPath);
  const session = readSession(rootPath);
  const taskGraph = readTaskGraph(rootPath);
  const repairs = [
    ...buildParallelGroupRepairs(taskGraph, specs),
  ];

  return {
    root_dir: rootPath.replace(/\\/g, "/"),
    inspected_at: nowIso(),
    repairable: repairs.length > 0,
    repair_count: repairs.length,
    repairs,
    session_run_id: session?.run_id || null,
  };
}

function applyParallelGroupRepairs(taskGraph, repairs) {
  const targetIds = new Set(
    repairs
      .filter((entry) => entry.repair_kind === "clear_parallel_group_v1")
      .map((entry) => String(entry.target || "").replace(/^node:/, "")),
  );

  if (targetIds.size === 0) {
    return clone(taskGraph);
  }

  return {
    ...clone(taskGraph),
    nodes: (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).map((node) => (
      targetIds.has(node.id) ? { ...node, parallel_group: null } : clone(node)
    )),
  };
}

function applyRuntimeStateRepair(rootDir, options = {}) {
  const rootPath = path.resolve(rootDir);
  const inspection = inspectRuntimeStateRepair(rootPath, options);
  const session = readSession(rootPath);
  const taskGraph = readTaskGraph(rootPath);

  if (!inspection.repairable) {
    return {
      ...inspection,
      applied: false,
      files_updated: [],
      journal_event: null,
    };
  }

  const nextTaskGraph = applyParallelGroupRepairs(taskGraph, inspection.repairs);
  const nextSession = {
    ...clone(session),
    timestamps: {
      ...(session?.timestamps || {}),
      updated_at: nowIso(),
    },
  };

  writeTaskGraph(rootPath, nextTaskGraph);
  writeSession(rootPath, nextSession);

  const repairEvent = appendEvent(rootPath, {
    runId: session?.run_id,
    event: "runtime_state_repaired",
    phase: session?.phase?.current || "none",
    nodeId: session?.node?.active_id || "none",
    actor: "runtime",
    payload: {
      repair_count: inspection.repairs.length,
      repairs: inspection.repairs.map((entry) => ({
        repair_kind: entry.repair_kind,
        target: entry.target,
        path: entry.path,
        reason: entry.reason,
      })),
    },
  });

  return {
    ...inspection,
    applied: true,
    files_updated: [
      ".ai/workflow/task-graph.yaml",
      ".ai/workflow/session.yaml",
      ".ai/workflow/journal.jsonl",
    ],
    journal_event: repairEvent.event,
  };
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

function formatHumanSummary(result) {
  if (!result.repairable) {
    return "[runtime-state-repair] 未发现可自动修复的运行态漂移。";
  }
  if (!result.applied) {
    return `[runtime-state-repair] 发现 ${result.repair_count} 项可自动修复漂移；使用 --write 应用。`;
  }
  return `[runtime-state-repair] 已应用 ${result.repair_count} 项运行态修复。`;
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
    const result = parsed.write ? applyRuntimeStateRepair(parsed.rootDir) : inspectRuntimeStateRepair(parsed.rootDir);
    if (parsed.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }
    console.log(formatHumanSummary(result));
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  applyRuntimeStateRepair,
  buildParallelGroupRepairs,
  inspectRuntimeStateRepair,
  parseArgs,
};

if (require.main === module) {
  main();
}

