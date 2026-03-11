#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const path = require("node:path");

const {
  buildFixtureState,
  assertSubset,
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
const { applyReviewDecision } = require("../../scripts/runtime/review-resolution.cjs");

function writeRuntimeState(rootDir, fixture) {
  const state = buildFixtureState(fixture);
  writeSession(rootDir, state.session);
  writeTaskGraph(rootDir, state.taskGraph);
  writeSprintStatus(rootDir, state.sprintStatus);
  return state;
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

const cases = {
  "record-spec-review-pass-refreshes-quality-review-handoff": () => {
    const rootDir = makeTempRoot("review-resolve-spec-pass-");
    fs.cpSync(path.resolve(__dirname, "..", ".."), rootDir, { recursive: true });
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

    const result = applyReviewDecision(rootDir, {
      decision: "pass",
      persona: "spec_reviewer",
      actor: "spec_reviewer",
      reason: "spec accepted",
    });

    assertSubset(result, {
      decision: "pass",
      reviewer_persona: "spec_reviewer",
      target_node_id: "P2-N1",
      route_verdict: "handoff",
      recommended_next: [{ type: "resume_node", target: "P2-N1" }],
    });

    const taskGraph = readTaskGraph(rootDir);
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    assertSubset(node, {
      review_state: { spec_review: "pass", quality_review: "pending" },
    });

    const session = readSession(rootDir);
    assertSubset(session, {
      node: { active_id: "P2-N1", owner_persona: "spec_reviewer" },
    });
    if (!session?.recovery?.last_checkpoint_id) {
      throw new Error("expected review checkpoint to be created");
    }

    const journal = readJournalEvents(rootDir);
    const verdictEvent = journal.find((item) => item.event === "review_verdict_recorded");
    if (!verdictEvent) {
      throw new Error("expected review_verdict_recorded event");
    }
    assertSubset(verdictEvent, {
      actor: "spec_reviewer",
      node_id: "P2-N1",
      payload: { reviewer_persona: "spec_reviewer", decision: "pass" },
    });
  },
  "record-quality-review-rework-refreshes-author-rework-route": () => {
    const rootDir = makeTempRoot("review-resolve-quality-rework-");
    fs.cpSync(path.resolve(__dirname, "..", ".."), rootDir, { recursive: true });
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
          review_state: { spec_review: "pass", quality_review: "pending" },
          evidence_refs: [".ai/analysis/ai.report.json"],
        },
      },
    });
    writeReadyReport(rootDir);

    const result = applyReviewDecision(rootDir, {
      decision: "rework",
      persona: "quality_reviewer",
      actor: "quality_reviewer",
      reason: "needs rework",
    });

    assertSubset(result, {
      decision: "rework",
      reviewer_persona: "quality_reviewer",
      target_node_id: "P2-N1",
      route_verdict: "block",
      recommended_next: [{ type: "resume_node", target: "P2-N1" }],
    });

    const taskGraph = readTaskGraph(rootDir);
    const node = taskGraph.nodes.find((item) => item.id === "P2-N1");
    assertSubset(node, {
      review_state: { spec_review: "pass", quality_review: "rework" },
    });

    const journal = readJournalEvents(rootDir);
    const verdictEvent = journal.find((item) => item.event === "review_verdict_recorded");
    if (!verdictEvent) {
      throw new Error("expected review_verdict_recorded event");
    }
    assertSubset(verdictEvent, {
      actor: "quality_reviewer",
      payload: { reviewer_persona: "quality_reviewer", decision: "rework" },
    });
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
  console.log("REVIEW_RESOLUTION_FIXTURES_PASS");
}

main();
