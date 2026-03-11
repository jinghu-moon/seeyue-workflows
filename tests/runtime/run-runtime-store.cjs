#!/usr/bin/env node
    "use strict";

    const fs = require("node:fs");
    const os = require("node:os");
    const path = require("node:path");

    const {
      writeSession,
      readSession,
      writeTaskGraph,
      readTaskGraph,
      writeSprintStatus,
      readSprintStatus,
      appendJournalEvents,
      readJournalEvents,
      writeLedger,
      readLedger,
      writeCheckpoint,
      readCheckpoint,
    } = require("../../scripts/runtime/store.cjs");

    function makeTempRoot() {
      return fs.mkdtempSync(path.join(os.tmpdir(), "sy-runtime-store-"));
    }

    function assert(condition, message) {
      if (!condition) {
        throw new Error(message);
      }
    }

    function createSession() {
      return {
        schema: 4,
        run_id: "wf-20260308-001",
        engine: { kind: "codex", adapter_version: 1 },
        task: { id: "task-1", title: "Runtime store", mode: "feature" },
        phase: { current: "design", status: "in_progress" },
        node: { active_id: "P1-N3", state: "idle", owner_persona: "planner" },
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
        recovery: { last_checkpoint_id: null, restore_pending: false, restore_reason: null },
        timestamps: {
          created_at: "2026-03-08T00:00:00Z",
          updated_at: "2026-03-08T00:00:00Z",
        },
      };
    }

    function createTaskGraph() {
      return {
        schema: 4,
        graph_id: "graph-1",
        phases: [
          {
            id: "P1",
            title: "Foundation",
            status: "in_progress",
            depends_on: [],
            entry_condition: ["design approved"],
            exit_gate: { cmd: "node test", pass_signal: "PASS", coverage_min: "80%" },
            rollback_boundary: { revert_nodes: ["P1-N1"], restore_point: "docs only" },
          },
        ],
        nodes: [
          {
            id: "P1-N3",
            phase_id: "P1",
            title: "Runtime store",
            target: "scripts/runtime/store.cjs",
            action: "Implement read/write store",
            why: "Need durable state access",
            depends_on: ["P1-N2"],
            verify: { cmd: "node tests/runtime/run-runtime-store.cjs", pass_signal: "RUNTIME_STORE_PASS" },
            risk_level: "high",
            tdd_required: true,
            status: "in_progress",
            tdd_state: "red_pending",
            owner_persona: "author",
            review_state: { spec_review: "pending", quality_review: "pending" },
            evidence_refs: [],
            output_refs: [],
            approval_ref: null,
            capability: "code_edit",
            priority: "high",
            condition: null,
            retry_policy: {
              max_attempts: 2,
              backoff_mode: "fixed",
              initial_delay_seconds: 1,
              max_delay_seconds: 1,
              retry_on: ["transient_tool_failure"],
            },
            timeout_policy: {
              timeout_seconds: 30,
              grace_seconds: 5,
              on_timeout: "block_node",
            },
            test_contract: {
              layer: "unit",
              coverage_mode: "patch",
              coverage_profile: "core",
              mock_policy: "none",
              acceptance_criteria_refs: ["AC-1"],
              red_cmd: "node tests/runtime/run-runtime-store.cjs --case atomic-write-recovery",
              green_cmd: "node tests/runtime/run-runtime-store.cjs",
              red_expectation: "atomic write failure preserves previous file",
              behavior_gate: "runtime store preserves durable state",
            },
          },
        ],
      };
    }

    function createSprintStatus() {
      return {
        schema: 4,
        active_phase: "P1",
        node_summary: [
          { id: "P1-N1", status: "completed", tdd_state: "not_applicable" },
          { id: "P1-N3", status: "in_progress", tdd_state: "red_pending" },
        ],
        recommended_next: [
          {
            type: "resume_node",
            target: "P1-N3",
            params: { mode: "test" },
            reason: "Need failing test evidence",
            blocking_on: [],
            priority: "now",
          },
        ],
      };
    }

    const cases = {
      "session-round-trip": () => {
        const rootDir = makeTempRoot();
        const expected = createSession();
        writeSession(rootDir, expected);
        const actual = readSession(rootDir);
        assert(actual.run_id === expected.run_id, "session run_id mismatch");
        assert(actual.workspace.root === expected.workspace.root, "session workspace mismatch");
      },
      "task-graph-round-trip": () => {
        const rootDir = makeTempRoot();
        const expected = createTaskGraph();
        writeTaskGraph(rootDir, expected);
        const actual = readTaskGraph(rootDir);
        assert(actual.graph_id === expected.graph_id, "task graph id mismatch");
        assert(actual.nodes[0].capability === "code_edit", "task graph capability mismatch");
      },
      "sprint-status-round-trip": () => {
        const rootDir = makeTempRoot();
        const expected = createSprintStatus();
        writeSprintStatus(rootDir, expected);
        const actual = readSprintStatus(rootDir);
        assert(actual.active_phase == "P1", "active phase mismatch");
        assert(actual.recommended_next[0].type === "resume_node", "recommended_next mismatch");
      },
      "journal-append-and-read": () => {
        const rootDir = makeTempRoot();
        appendJournalEvents(rootDir, [
          { event: "phase_started", payload: { phase_id: "P1" }, trace_id: "t-1" },
          { event: "node_started", payload: { node_id: "P1-N3" }, trace_id: "t-2" },
        ]);
        const items = readJournalEvents(rootDir);
        assert(items.length === 2, "journal length mismatch");
        assert(items[1].event === "node_started", "journal event mismatch");
      },
      "journal-append-multi-call": () => {
        const rootDir = makeTempRoot();
        appendJournalEvents(rootDir, [
          { event: "phase_started", payload: { phase_id: "P1" }, trace_id: "t-1" },
        ]);
        appendJournalEvents(rootDir, [
          { event: "node_started", payload: { node_id: "P1-N3" }, trace_id: "t-2" },
          { event: "node_completed", payload: { node_id: "P1-N3" }, trace_id: "t-3" },
        ]);
        const items = readJournalEvents(rootDir);
        assert(items.length === 3, "journal multi-call length mismatch");
        assert(items[2].event === "node_completed", "journal multi-call order mismatch");
      },
      "journal-append-lock-stale-recovery": () => {
        const rootDir = makeTempRoot();
        const journalPath = path.join(rootDir, ".ai", "workflow", "journal.jsonl");
        const lockPath = `${journalPath}.lock`;
        fs.mkdirSync(path.dirname(journalPath), { recursive: true });
        const staleMeta = JSON.stringify({ pid: 999999, created_at: new Date(Date.now() - 60000).toISOString() });
        fs.writeFileSync(lockPath, `${staleMeta}\n`, "utf8");
        appendJournalEvents(rootDir, [
          { event: "phase_started", payload: { phase_id: "P1" }, trace_id: "t-1" },
        ]);
        const items = readJournalEvents(rootDir);
        assert(items.length === 1, "journal stale lock recovery should append");
        assert(items[0].event === "phase_started", "journal stale lock recovery event mismatch");
      },
      "journal-append-too-large": () => {
        const rootDir = makeTempRoot();
        const payload = "x".repeat(5000);
        let threw = false;
        try {
          appendJournalEvents(rootDir, [{ event: "oversize", payload, trace_id: "t-oversize" }]);
        } catch (error) {
          threw = true;
        }
        assert(threw, "expected oversize journal payload to throw");
      },
      "ledger-round-trip": () => {
        const rootDir = makeTempRoot();
        const content = [
          "# Current Run",
          "",
          "- run_id: wf-20260308-001",
          "",
          "## Recommended Next",
          "- resume P1-N3",
          "",
        ].join("\n");
        writeLedger(rootDir, content);
        const actual = readLedger(rootDir);
        assert(actual.includes("Recommended Next"), "ledger content mismatch");
      },
      "checkpoint-round-trip": () => {
        const rootDir = makeTempRoot();
        const checkpoint = {
          checkpoint_id: "cp-1",
          run_id: "wf-20260308-001",
          phase: "P1",
          node_id: "P1-N3",
          session_snapshot_ref: ".ai/workflow/session.yaml",
          task_graph_snapshot_ref: ".ai/workflow/task-graph.yaml",
          journal_offset: 2,
          integrity_hash: "abc123",
          restore_status: "not_restored",
          created_at: "2026-03-08T00:00:00Z",
          restore_verified_at: null,
        };
        writeCheckpoint(rootDir, checkpoint);
        const actual = readCheckpoint(rootDir, checkpoint.checkpoint_id);
        assert(actual.checkpoint_id === checkpoint.checkpoint_id, "checkpoint id mismatch");
      },
      "atomic-write-recovery": () => {
        const rootDir = makeTempRoot();
        const original = createSession();
        const updated = createSession();
        updated.phase.current = "design";
        updated.phase.status = "review";
        writeSession(rootDir, original);
        let threw = false;
        try {
          writeSession(rootDir, updated, { injectFailure: "before_commit" });
        } catch (error) {
          threw = true;
        }
        assert(threw, "expected atomic write failure");
        const actual = readSession(rootDir);
        assert(actual.phase.status === original.phase.status, "session should remain original after failed commit");
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
      console.log("RUNTIME_STORE_PASS");
    }

    main();
