#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const { appendEvent } = require("./journal.cjs");
const { refreshLedger } = require("./ledger.cjs");
const { priorityRank } = require("./runtime-state.cjs");
const {
  ensureRuntimeLayout,
  readSession,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("./store.cjs");
const { loadYamlFile } = require("./yaml-loader.cjs");

const ANALYSIS_FILES = ["ai.report.json", "verify-staging.json", "coverage-staging.json"];
const TASK_MODES = new Set(["feature", "bugfix", "refactor", "docs", "research"]);

function nowIso() {
  return new Date().toISOString();
}

function normalizeRoot(rootDir) {
  return path.resolve(rootDir).replace(/\\/g, "/");
}

function buildRunId(now = new Date()) {
  const yyyy = String(now.getFullYear());
  const mm = String(now.getMonth() + 1).padStart(2, "0");
  const dd = String(now.getDate()).padStart(2, "0");
  const hh = String(now.getHours()).padStart(2, "0");
  const min = String(now.getMinutes()).padStart(2, "0");
  const sec = String(now.getSeconds()).padStart(2, "0");
  const ms = String(now.getMilliseconds()).padStart(3, "0");
  return `wf-${yyyy}${mm}${dd}-${hh}${min}${sec}${ms}`;
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function removeIfExists(targetPath) {
  if (!fs.existsSync(targetPath)) {
    return;
  }
  fs.rmSync(targetPath, { recursive: true, force: true });
}

function copyIfExists(sourcePath, targetPath) {
  if (!fs.existsSync(sourcePath)) {
    return false;
  }
  ensureDir(path.dirname(targetPath));
  const stat = fs.statSync(sourcePath);
  if (stat.isDirectory()) {
    fs.cpSync(sourcePath, targetPath, { recursive: true });
    return true;
  }
  fs.copyFileSync(sourcePath, targetPath);
  return true;
}

function runtimePaths(rootDir) {
  const absoluteRoot = path.resolve(rootDir);
  return {
    root: absoluteRoot,
    ai: path.join(absoluteRoot, ".ai"),
    workflow: path.join(absoluteRoot, ".ai", "workflow"),
    analysis: path.join(absoluteRoot, ".ai", "analysis"),
    archiveRoot: path.join(absoluteRoot, ".ai", "archive"),
    session: path.join(absoluteRoot, ".ai", "workflow", "session.yaml"),
    taskGraph: path.join(absoluteRoot, ".ai", "workflow", "task-graph.yaml"),
    sprintStatus: path.join(absoluteRoot, ".ai", "workflow", "sprint-status.yaml"),
    journal: path.join(absoluteRoot, ".ai", "workflow", "journal.jsonl"),
    ledger: path.join(absoluteRoot, ".ai", "workflow", "ledger.md"),
    capsules: path.join(absoluteRoot, ".ai", "workflow", "capsules"),
    checkpoints: path.join(absoluteRoot, ".ai", "workflow", "checkpoints"),
  };
}

function assertBootstrapAllowed(session) {
  if (!session) {
    return;
  }
  const completed = session?.phase?.status === "completed";
  const nodeCleared = session?.node?.active_id === "none";
  const approvalsClear = session?.approvals?.pending !== true && Number(session?.approvals?.pending_count || 0) === 0;
  const recoveryClear = session?.recovery?.restore_pending !== true;
  if (!completed || !nodeCleared || !approvalsClear || !recoveryClear) {
    throw new Error("active run is not in a clean terminal handoff state; archive/bootstrap refused");
  }
}

function archiveCurrentRun(rootDir, session) {
  if (!session?.run_id) {
    return { archived: false, archiveDir: null, manifest: null };
  }

  const paths = runtimePaths(rootDir);
  const archiveDir = path.join(paths.archiveRoot, session.run_id);
  if (fs.existsSync(archiveDir)) {
    throw new Error(`archive target already exists: ${archiveDir}`);
  }

  ensureDir(archiveDir);
  const archivedPaths = [];
  const workflowTargets = [
    [paths.session, path.join(archiveDir, "workflow", "session.yaml")],
    [paths.taskGraph, path.join(archiveDir, "workflow", "task-graph.yaml")],
    [paths.sprintStatus, path.join(archiveDir, "workflow", "sprint-status.yaml")],
    [paths.journal, path.join(archiveDir, "workflow", "journal.jsonl")],
    [paths.ledger, path.join(archiveDir, "workflow", "ledger.md")],
    [paths.capsules, path.join(archiveDir, "workflow", "capsules")],
    [paths.checkpoints, path.join(archiveDir, "workflow", "checkpoints")],
  ];

  for (const [sourcePath, targetPath] of workflowTargets) {
    if (copyIfExists(sourcePath, targetPath)) {
      archivedPaths.push(path.relative(paths.root, targetPath).replace(/\\/g, "/"));
    }
  }

  for (const fileName of ANALYSIS_FILES) {
    const sourcePath = path.join(paths.analysis, fileName);
    const targetPath = path.join(archiveDir, "analysis", fileName);
    if (copyIfExists(sourcePath, targetPath)) {
      archivedPaths.push(path.relative(paths.root, targetPath).replace(/\\/g, "/"));
    }
  }

  const manifest = {
    archived_at: nowIso(),
    archived_run_id: session.run_id,
    archived_paths: archivedPaths,
  };
  ensureDir(path.join(archiveDir, "meta"));
  fs.writeFileSync(path.join(archiveDir, "meta", "manifest.json"), JSON.stringify(manifest, null, 2), "utf8");

  return {
    archived: true,
    archiveDir: archiveDir.replace(/\\/g, "/"),
    manifest,
  };
}

function clearActiveRuntime(rootDir) {
  const paths = runtimePaths(rootDir);
  removeIfExists(paths.session);
  removeIfExists(paths.taskGraph);
  removeIfExists(paths.sprintStatus);
  removeIfExists(paths.journal);
  removeIfExists(paths.ledger);
  removeIfExists(paths.capsules);
  removeIfExists(paths.checkpoints);

  for (const fileName of ANALYSIS_FILES) {
    removeIfExists(path.join(paths.analysis, fileName));
  }
}

function loadGraphTemplate(rootDir, graphPath) {
  if (graphPath) {
    return loadYamlFile(path.resolve(rootDir, graphPath));
  }
  const existing = readTaskGraph(rootDir);
  if (!existing) {
    throw new Error("task graph template not found; pass --graph or create .ai/workflow/task-graph.yaml first");
  }
  return existing;
}

function buildReviewState() {
  return {
    spec_review: "pending",
    quality_review: "pending",
  };
}

function resetTaskGraph(templateGraph) {
  const phases = Array.isArray(templateGraph?.phases) ? templateGraph.phases : [];
  const nodes = Array.isArray(templateGraph?.nodes) ? templateGraph.nodes : [];
  if (phases.length === 0) {
    throw new Error("graph template must contain at least one phase");
  }

  const firstPhase = phases[0];
  const resetPhases = phases.map((phase, index) => ({
    ...structuredClone(phase),
    status: index === 0 ? "in_progress" : "pending",
  }));

  const resetNodes = nodes.map((node) => {
    const dependencyIds = Array.isArray(node.depends_on) ? node.depends_on : [];
    const ready = node.phase_id === firstPhase.id && dependencyIds.length === 0;
    return {
      ...structuredClone(node),
      status: ready ? "ready" : "pending",
      tdd_state: node.tdd_required === false ? "not_applicable" : "red_pending",
      owner_persona: "author",
      review_state: buildReviewState(),
      evidence_refs: [],
      approval_ref: null,
      parallel_group: node.parallel_group ?? null,
    };
  });

  return {
    graph: {
      ...structuredClone(templateGraph),
      schema: 4,
      phases: resetPhases,
      nodes: resetNodes,
    },
    initialPhaseId: firstPhase.id,
  };
}

function selectRecommendedNext(taskGraph, activePhaseId) {
  const nodes = (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : [])
    .filter((node) => node.phase_id === activePhaseId && node.status === "ready")
    .sort((left, right) => {
      if (priorityRank(left.priority) !== priorityRank(right.priority)) {
        return priorityRank(left.priority) - priorityRank(right.priority);
      }
      return String(left.id || "").localeCompare(String(right.id || ""));
    });

  const first = nodes[0];
  if (!first) {
    return [{
      type: "human_intervention",
      target: activePhaseId,
      params: {},
      reason: "bootstrap produced no ready node",
      blocking_on: [],
      priority: "now",
    }];
  }
  return [{
    type: "start_node",
    target: first.id,
    params: {},
    reason: "start highest priority ready node after bootstrap",
    blocking_on: [],
    priority: "now",
  }];
}

function buildLoopBudget(previousSession) {
  return {
    max_nodes: Number(previousSession?.loop_budget?.max_nodes || 24),
    max_failures: Number(previousSession?.loop_budget?.max_failures || 2),
    max_pending_approvals: Number(previousSession?.loop_budget?.max_pending_approvals || 2),
    consumed_nodes: 0,
    consumed_failures: 0,
  };
}

function buildContextBudget(previousSession) {
  return {
    strategy: previousSession?.context_budget?.strategy || "hybrid",
    capsule_refresh_threshold: Number(previousSession?.context_budget?.capsule_refresh_threshold || 4),
    summary_required_after_turns: Number(previousSession?.context_budget?.summary_required_after_turns || 8),
  };
}

function buildWorkspace(rootDir, previousSession) {
  return {
    root: normalizeRoot(rootDir),
    sandbox_mode: previousSession?.workspace?.sandbox_mode || "workspace_write",
  };
}

function buildSession(rootDir, options, previousSession, initialPhaseId, runId) {
  const timestamp = nowIso();
  return {
    schema: 4,
    run_id: runId,
    engine: {
      kind: options.engineKind || previousSession?.engine?.kind || "codex",
      adapter_version: Number(previousSession?.engine?.adapter_version || 1),
    },
    task: {
      id: options.taskId,
      title: options.taskTitle,
      mode: options.taskMode,
    },
    phase: {
      current: initialPhaseId,
      status: "in_progress",
    },
    node: {
      active_id: "none",
      state: "idle",
      owner_persona: "planner",
    },
    loop_budget: buildLoopBudget(previousSession),
    context_budget: buildContextBudget(previousSession),
    workspace: buildWorkspace(rootDir, previousSession),
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
      created_at: timestamp,
      updated_at: timestamp,
    },
  };
}

function buildSprintStatus(taskGraph, activePhaseId, recommendedNext) {
  return {
    schema: 4,
    active_phase: activePhaseId,
    node_summary: (Array.isArray(taskGraph?.nodes) ? taskGraph.nodes : []).map((node) => ({
      id: node.id,
      status: node.status,
      tdd_state: node.tdd_state,
    })),
    recommended_next: recommendedNext,
  };
}

function bootstrapRun(rootDir, options) {
  if (!options.taskId || !options.taskTitle || !options.taskMode) {
    throw new Error("bootstrap requires --task-id, --task-title, and --task-mode");
  }
  if (!TASK_MODES.has(options.taskMode)) {
    throw new Error(`invalid --task-mode: ${options.taskMode}`);
  }

  const previousSession = readSession(rootDir);
  assertBootstrapAllowed(previousSession);
  const archive = previousSession ? archiveCurrentRun(rootDir, previousSession) : { archived: false, archiveDir: null, manifest: null };
  const graphTemplate = loadGraphTemplate(rootDir, options.graphPath);
  const { graph, initialPhaseId } = resetTaskGraph(graphTemplate);
  const newRunId = buildRunId();

  clearActiveRuntime(rootDir);
  ensureRuntimeLayout(rootDir);

  const recommendedNext = selectRecommendedNext(graph, initialPhaseId);
  const session = buildSession(rootDir, options, previousSession, initialPhaseId, newRunId);
  const sprintStatus = buildSprintStatus(graph, initialPhaseId, recommendedNext);

  writeSession(rootDir, session);
  writeTaskGraph(rootDir, graph);
  writeSprintStatus(rootDir, sprintStatus);
  appendEvent(rootDir, {
    runId: newRunId,
    event: "session_started",
    phase: initialPhaseId,
    nodeId: "none",
    actor: "runtime",
    payload: {
      source: "runtime_bootstrap",
      archived_run_id: previousSession?.run_id || null,
      task_id: options.taskId,
    },
  });
  appendEvent(rootDir, {
    runId: newRunId,
    event: "phase_entered",
    phase: initialPhaseId,
    nodeId: "none",
    actor: "runtime",
    payload: {
      source: "runtime_bootstrap",
      archived_run_id: previousSession?.run_id || null,
    },
  });
  refreshLedger(rootDir);

  return {
    root_dir: normalizeRoot(rootDir),
    archived_run_id: previousSession?.run_id || null,
    archive_dir: archive.archiveDir,
    new_run_id: newRunId,
    initial_phase: initialPhaseId,
    recommended_next: recommendedNext,
  };
}

function parseArgs(argv) {
  const parsed = {
    rootDir: path.resolve(__dirname, "..", ".."),
    taskId: null,
    taskTitle: null,
    taskMode: null,
    graphPath: null,
    engineKind: null,
    json: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    switch (token) {
      case "--root":
        index += 1;
        parsed.rootDir = path.resolve(argv[index]);
        break;
      case "--task-id":
        index += 1;
        parsed.taskId = argv[index];
        break;
      case "--task-title":
        index += 1;
        parsed.taskTitle = argv[index];
        break;
      case "--task-mode":
        index += 1;
        parsed.taskMode = argv[index];
        break;
      case "--graph":
        index += 1;
        parsed.graphPath = argv[index];
        break;
      case "--engine-kind":
        index += 1;
        parsed.engineKind = argv[index];
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

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (error) {
    console.error(`ARG_PARSE_FAIL ${error.message}`);
    process.exit(1);
  }

  try {
    const result = bootstrapRun(parsed.rootDir, parsed);
    if (parsed.json) {
      console.log(JSON.stringify(result, null, 2));
      return;
    }
    console.log(`[runtime-bootstrap] new_run_id=${result.new_run_id} archived=${result.archived_run_id || "none"} next=${result.recommended_next[0]?.type || "none"}:${result.recommended_next[0]?.target || "none"}`);
  } catch (error) {
    console.error(error.stack || error.message);
    process.exit(1);
  }
}

module.exports = {
  archiveCurrentRun,
  bootstrapRun,
  buildRunId,
  clearActiveRuntime,
  parseArgs,
  resetTaskGraph,
  selectRecommendedNext,
};

if (require.main === module) {
  main();
}
