#!/usr/bin/env node
"use strict";

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

function runHook(projectRoot, scriptPath, payload) {
  const run = spawnSync(process.execPath, [scriptPath], {
    cwd: projectRoot,
    input: JSON.stringify(payload),
    encoding: "utf8",
  });
  return {
    code: Number.isInteger(run.status) ? run.status : 1,
    stdout: String(run.stdout || "").trim(),
    stderr: String(run.stderr || "").trim(),
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

function nowIso() {
  return new Date().toISOString();
}

function ensureDir(dirPath) {
  fs.mkdirSync(dirPath, { recursive: true });
}

function writeJson(filePath, value) {
  ensureDir(path.dirname(filePath));
  fs.writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function getSessionPath(workspace, format = "yaml") {
  const ext = format === "md" ? "md" : "yaml";
  return path.join(workspace, ".ai", "workflow", `session.${ext}`);
}

function writeWorkflowSession(workspace, phase, nextAction, format = "yaml") {
  const sessionPath = getSessionPath(workspace, format);
  ensureDir(path.dirname(sessionPath));
  const body = [
    "run_id: wf-20260307-001",
    `current_phase: ${phase}`,
    `next_action: ${nextAction}`,
    `updated_at: ${nowIso()}`,
    "",
  ].join("\n");
  fs.writeFileSync(sessionPath, body, "utf8");
}

function writeSessionWithNode(workspace, options = {}) {
  const sessionPath = getSessionPath(workspace, options.format);
  ensureDir(path.dirname(sessionPath));
  const lines = [
    `run_id: ${options.runId || "wf-20260307-001"}`,
    `current_phase: ${options.phase || "execute"}`,
    `next_action: ${options.nextAction || "/execute verify N1"}`,
    `last_completed_node: ${options.lastCompletedNode || "none"}`,
    `total_nodes: ${options.totalNodes || 1}`,
    `mode: ${options.mode || "normal"}`,
    `tdd_required: ${options.tddRequired || "false"}`,
    `red_verified: ${options.redVerified || "false"}`,
    `updated_at: ${options.updatedAt || nowIso()}`,
    "",
  ];
  fs.writeFileSync(sessionPath, lines.join("\n"), "utf8");
}

function writePassReport(workspace, overrides = {}) {
  const reportPath = path.join(workspace, ".ai", "analysis", "ai.report.json");
  writeJson(reportPath, {
    report_name: "ai.report",
    report_version: "test",
    generated_at: nowIso(),
    updated_at: nowIso(),
    delta_basis: { changed_files: 1 },
    verification: {
      compile: "pass",
      test: "pass",
      lint: "skip",
      build: "skip",
    },
    ...overrides,
  });
}

function createTempWorkspace() {
  return fs.mkdtempSync(path.join(os.tmpdir(), "sy-hooks-smoke-"));
}

function main() {
  const projectRoot = path.resolve(__dirname, "..", "..");
  const hookDir = path.resolve(projectRoot, "scripts", "hooks");
  const beforeToolSelection = path.join(hookDir, "sy-before-tool-selection.cjs");
  const afterModel = path.join(hookDir, "sy-after-model.cjs");
  const pretoolBash = path.join(hookDir, "sy-pretool-bash.cjs");
  const pretoolWrite = path.join(hookDir, "sy-pretool-write.cjs");
  const pretoolWriteSession = path.join(hookDir, "sy-pretool-write-session.cjs");
  const posttoolBashVerify = path.join(hookDir, "sy-posttool-bash-verify.cjs");
  const stop = path.join(hookDir, "sy-stop.cjs");

  const cases = [];

  cases.push({
    name: "pretool_bash_allow_safe_command",
    run: () =>
      runHook(projectRoot, pretoolBash, {
        tool_name: "Bash",
        tool_input: { command: "echo safe" },
      }),
    expectCode: 0,
  });

  cases.push({
    name: "pretool_bash_block_force_push",
    run: () =>
      runHook(projectRoot, pretoolBash, {
        tool_name: "Bash",
        tool_input: { command: "git push --force origin main" },
      }),
    expectCode: 2,
  });

  cases.push({
    name: "pretool_write_allow_env_reference",
    run: () =>
      runHook(projectRoot, pretoolWrite, {
        tool_name: "Write",
        tool_input: {
          file_path: "src/example.ts",
          content: "const token = process.env.API_TOKEN;\n",
        },
      }),
    expectCode: 0,
  });

  cases.push({
    name: "pretool_write_block_hardcoded_token",
    run: () =>
      runHook(projectRoot, pretoolWrite, {
        tool_name: "Write",
        tool_input: {
          file_path: "src/example.ts",
          content: "const token = 'ghp_abcdefghijklmnopqrstuvwxyz012345';\n",
        },
      }),
    expectCode: 2,
  });

  cases.push({
    name: "pretool_write_block_by_tdd_red_gate",
    run: () => {
      const workspace = createTempWorkspace();
      writeSessionWithNode(workspace, {
        phase: "execute",
        tddRequired: "true",
        redVerified: "false",
      });
      return runHook(projectRoot, pretoolWrite, {
        cwd: workspace,
        tool_name: "Write",
        tool_input: {
          file_path: "src/app.ts",
          content: "export const v = 1;\n",
        },
      });
    },
    expectCode: 2,
  });

  cases.push({
    name: "before_tool_selection_read_only_in_plan",
    run: () => {
      const workspace = createTempWorkspace();
      writeWorkflowSession(workspace, "plan", "/plan");
      const result = runHook(projectRoot, beforeToolSelection, {
        cwd: workspace,
        llm_request: { messages: [] },
      });
      const output = parseHookOutput(result.stdout);
      const allowed = output?.hookSpecificOutput?.toolConfig?.allowedFunctionNames || [];
      const ok = output.verdict === "allow"
        && Array.isArray(allowed)
        && allowed.includes("read_file")
        && !allowed.includes("run_shell_command");
      return { ...result, code: ok ? 0 : 1 };
    },
    expectCode: 0,
  });

  cases.push({
    name: "after_model_redacts_secret",
    run: () => {
      const result = runHook(projectRoot, afterModel, {
        llm_response: {
          candidates: [
            {
              content: {
                role: "model",
                parts: ["Token: ghp_abcdefghijklmnopqrstuvwxyz012345"],
              },
              finishReason: "STOP",
            },
          ],
        },
      });
      const output = parseHookOutput(result.stdout);
      const redacted = output?.hookSpecificOutput?.llm_response?.candidates?.[0]?.content?.parts?.[0];
      const ok = typeof redacted === "string"
        && redacted.includes("[REDACTED]")
        && !redacted.includes("ghp_");
      return { ...result, code: ok ? 0 : 1 };
    },
    expectCode: 0,
  });

  cases.push({
    name: "after_model_redacts_text_field",
    run: () => {
      const result = runHook(projectRoot, afterModel, {
        llm_response: {
          text: "Bearer ghp_abcdefghijklmnopqrstuvwxyz012345",
          candidates: [],
        },
      });
      const output = parseHookOutput(result.stdout);
      const redacted = output?.hookSpecificOutput?.llm_response?.text;
      const ok = typeof redacted === "string"
        && redacted.includes("[REDACTED]")
        && !redacted.includes("ghp_");
      return { ...result, code: ok ? 0 : 1 };
    },
    expectCode: 0,
  });

  cases.push({
    name: "after_model_redacts_part_object",
    run: () => {
      const result = runHook(projectRoot, afterModel, {
        llm_response: {
          candidates: [
            {
              content: {
                role: "model",
                parts: [{ text: "token: ghp_abcdefghijklmnopqrstuvwxyz012345" }],
              },
              finishReason: "STOP",
            },
          ],
        },
      });
      const output = parseHookOutput(result.stdout);
      const redacted = output?.hookSpecificOutput?.llm_response?.candidates?.[0]?.content?.parts?.[0]?.text;
      const ok = typeof redacted === "string"
        && redacted.includes("[REDACTED]")
        && !redacted.includes("ghp_");
      return { ...result, code: ok ? 0 : 1 };
    },
    expectCode: 0,
  });

  cases.push({
    name: "pretool_write_session_block_invalid_phase",
    run: () => {
      const workspace = createTempWorkspace();
      return runHook(projectRoot, pretoolWriteSession, {
        cwd: workspace,
        tool_name: "Write",
        tool_input: {
          file_path: ".ai/workflow/session.yaml",
          content: [
            "run_id: wf-20260307-001",
            "current_phase: wrongphase",
            "next_action: /execute node N1",
            `updated_at: ${nowIso()}`,
            "",
          ].join("\n"),
        },
      });
    },
    expectCode: 2,
  });

  cases.push({
    name: "pretool_write_session_block_invalid_v4_status",
    run: () => {
      const workspace = createTempWorkspace();
      return runHook(projectRoot, pretoolWriteSession, {
        cwd: workspace,
        tool_name: "Write",
        tool_input: {
          file_path: ".ai/workflow/session.yaml",
          content: [
            "schema: 4",
            "run_id: wf-20260307-001",
            "engine:",
            "  kind: codex",
            "  adapter_version: 1",
            "task:",
            "  id: task-p3",
            "  title: Hooks",
            "  mode: feature",
            "phase:",
            "  current: P3",
            "  status: wrong_status",
            "node:",
            "  active_id: P3-N2",
            "  state: red_pending",
            "  owner_persona: author",
            `timestamps:\n  created_at: ${nowIso()}\n  updated_at: ${nowIso()}`,
            "",
          ].join("\n"),
        },
      });
    },
    expectCode: 2,
  });

  cases.push({
    name: "posttool_bash_verify_capture_writes_staging",
    run: () => {
      const workspace = createTempWorkspace();
      writeSessionWithNode(workspace, { phase: "execute" });
      const result = runHook(projectRoot, posttoolBashVerify, {
        cwd: workspace,
        tool_name: "Bash",
        tool_input: { command: "cargo test" },
        tool_response: {
          returncode: 0,
          stdout: "test result: ok. 3 passed; 0 failed;",
          stderr: "",
        },
      });
      const stagingPath = path.join(workspace, ".ai", "analysis", "verify-staging.json");
      const hasStaging = fs.existsSync(stagingPath);
      return { ...result, code: result.code === 0 && hasStaging ? 0 : 1 };
    },
    expectCode: 0,
  });

  cases.push({
    name: "stop_block_when_execute_checkpoint_incomplete",
    run: () => {
      const workspace = createTempWorkspace();
      writeSessionWithNode(workspace, {
        phase: "execute",
        lastCompletedNode: "N1",
        nextAction: "/execute verify N1",
      });
      const result = runHook(projectRoot, stop, {
        cwd: workspace,
      });
      const output = parseHookOutput(result.stdout);
      return { ...result, code: output.verdict === "force_continue" ? 0 : 1 };
    },
    expectCode: 0,
  });

  cases.push({
    name: "stop_allow_when_review_has_fresh_report",
    run: () => {
      const workspace = createTempWorkspace();
      writeSessionWithNode(workspace, {
        phase: "review",
        nextAction: "/review",
      });
      writePassReport(workspace);
      return runHook(projectRoot, stop, {
        cwd: workspace,
      });
    },
    expectCode: 0,
  });

  cases.push({
    name: "stop_allow_when_review_uses_legacy_session_md",
    run: () => {
      const workspace = createTempWorkspace();
      writeSessionWithNode(workspace, {
        phase: "review",
        nextAction: "/review",
        format: "md",
      });
      writePassReport(workspace);
      return runHook(projectRoot, stop, {
        cwd: workspace,
      });
    },
    expectCode: 0,
  });

  let pass = 0;
  let fail = 0;
  for (const testCase of cases) {
    const result = testCase.run();
    const ok = result.code === testCase.expectCode;
    if (ok) {
      pass += 1;
      console.log(`[PASS] ${testCase.name} (exit=${result.code})`);
      continue;
    }
    fail += 1;
    console.log(`[FAIL] ${testCase.name} expected=${testCase.expectCode} actual=${result.code}`);
    if (result.stderr) {
      console.log(`  stderr: ${result.stderr}`);
    }
    if (result.stdout) {
      console.log(`  stdout: ${result.stdout}`);
    }
  }

  console.log("=== Summary ===");
  console.log(`pass: ${pass}`);
  console.log(`fail: ${fail}`);
  console.log(`total: ${cases.length}`);

  process.exit(fail === 0 ? 0 : 1);
}

main();
