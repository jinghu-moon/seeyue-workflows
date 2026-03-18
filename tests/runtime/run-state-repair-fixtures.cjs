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
const {
  readJournalEvents,
  readSession,
  readTaskGraph,
  writeSession,
  writeSprintStatus,
  writeTaskGraph,
} = require("../../scripts/runtime/store.cjs");
const {
  applyRuntimeStateRepair,
  inspectRuntimeStateRepair,
} = require("../../scripts/runtime/state-repair.cjs");

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  return state;
}

function runController(rootDir, args) {
  const controllerPath = path.join(rootDir, "scripts", "runtime", "controller.cjs");
  return spawnSync(process.execPath, [controllerPath, ...args], {
    cwd: rootDir,
    encoding: "utf8",
  });
}

function parseJsonOutput(result, label) {
  if (result.status !== 0) {
    throw new Error(`${label} exited with ${result.status}: ${result.stderr || result.stdout}`);
  }
  return JSON.parse(result.stdout || "{}");
}

const cases = {
  "inspect-detects-parallel-group-drift": () => {
    const rootDir = makeTempRoot("runtime-repair-inspect-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      nodes: {
        "P2-N1": { parallel_group: "legacy_group" },
        "P2-N2": { parallel_group: "legacy_group" },
      },
    });

    const result = inspectRuntimeStateRepair(rootDir);
    assertSubset(result, {
      repairable: true,
      repair_count: 2,
      repairs: [
        { repair_kind: "clear_parallel_group_v1", target: "node:P2-N1", after: null },
        { repair_kind: "clear_parallel_group_v1", target: "node:P2-N2", after: null },
      ],
    });

    const taskGraph = readTaskGraph(rootDir);
    if (taskGraph.nodes.find((node) => node.id === "P2-N1")?.parallel_group !== "legacy_group") {
      throw new Error("inspect must not mutate task graph");
    }
  },
  "apply-clears-parallel-group-and-emits-journal": () => {
    const rootDir = makeTempRoot("runtime-repair-apply-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {
      nodes: {
        "P2-N1": { parallel_group: "legacy_group" },
      },
    });

    const beforeSession = readSession(rootDir);
    const result = applyRuntimeStateRepair(rootDir);
    assertSubset(result, {
      repairable: true,
      applied: true,
      repair_count: 1,
      files_updated: [
        ".ai/workflow/task-graph.yaml",
        ".ai/workflow/session.yaml",
        ".ai/workflow/journal.jsonl",
      ],
      journal_event: "runtime_state_repaired",
    });

    const taskGraph = readTaskGraph(rootDir);
    const repairedNode = taskGraph.nodes.find((node) => node.id === "P2-N1");
    if (!repairedNode || repairedNode.parallel_group !== null) {
      throw new Error("expected P2-N1 parallel_group to become null");
    }

    const afterSession = readSession(rootDir);
    if (afterSession.timestamps.updated_at === beforeSession.timestamps.updated_at) {
      throw new Error("expected session timestamp to update after repair");
    }

    const journal = readJournalEvents(rootDir);
    if (!journal.some((entry) => entry.event === "runtime_state_repaired")) {
      throw new Error("expected runtime_state_repaired event");
    }
  },
  "controller-verify-can-repair-before-evaluation": () => {
    const rootDir = makeTempRoot("runtime-repair-controller-");
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
          parallel_group: "legacy_group",
        },
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

    const result = parseJsonOutput(
      runController(rootDir, ["--root", rootDir, "--mode", "verify", "--repair-state", "--write-report", "--json"]),
      "controller",
    );

    assertSubset(result, {
      repair: {
        applied: true,
        repair_count: 1,
      },
      decision: {
        validator_verdict: { verdict: "PASS" },
        policy_verdict: { completion: { node_complete_ready: true } },
      },
      verification: {
        report_overall: "READY",
        review_ready: true,
      },
    });
  },

  "interaction-missing-defaults": () => {
    // RED: session has no interaction block → inspect must report repairable
    const rootDir = makeTempRoot("runtime-repair-interaction-missing-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {});
    const session = readSession(rootDir);
    // Strip interaction block if present
    const sessionWithout = { ...session };
    delete sessionWithout.interaction;
    writeSession(rootDir, sessionWithout);

    const result = inspectRuntimeStateRepair(rootDir);
    if (!result.repairable) {
      throw new Error("expected repairable=true when interaction block is missing");
    }
    const interactionRepair = result.repairs.find((r) => r.target === "session:interaction");
    if (!interactionRepair) {
      throw new Error("expected repair targeting session:interaction");
    }
  },

  "interaction-defaults": () => {
    // GREEN: apply repair → session gains interaction block with correct defaults
    const rootDir = makeTempRoot("runtime-repair-interaction-defaults-");
    copyRuntimeFixtureFiles(rootDir);
    writeRuntimeState(rootDir, {});
    const session = readSession(rootDir);
    const sessionWithout = { ...session };
    delete sessionWithout.interaction;
    writeSession(rootDir, sessionWithout);

    const result = applyRuntimeStateRepair(rootDir);
    if (!result.applied) {
      throw new Error("expected applied=true after repairing missing interaction block");
    }

    const repaired = readSession(rootDir);
    if (!repaired.interaction) {
      throw new Error("expected session.interaction to exist after repair");
    }
    if (repaired.interaction.pending_count !== 0) {
      throw new Error("expected interaction.pending_count=0");
    }
    if (repaired.interaction.active_interaction_id !== null) {
      throw new Error("expected interaction.active_interaction_id=null");
    }
    if (repaired.interaction.last_dispatched_at !== null) {
      throw new Error("expected interaction.last_dispatched_at=null");
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
    console.log("STATE_REPAIR_FIXTURES_PASS");
  }
}

main();
