---
name: sy-workflow
description: Use when users want a single entry to start, route, resume, or inspect the sy-* phased development workflow.
allowed-tools:
  - Read
argument-hint: "[command, task]"
disable-model-invocation: false
---

# SY Unified Workflow

## Overview

Single-entry orchestrator for the `sy-*` workflow stack.

## Trigger

Use when user requests:
- workflow start
- idea discovery or benchmark-based planning
- approved-design convergence before planning
- workspace/worktree preparation before implementation
- resume/status of previous workflow run
- debugging after verification/review failures
- review feedback processing through the workflow path
- explicit workflow closure after work is complete

## Entry Commands

| Trigger | Operation | Purpose |
|---|---|---|
| `工作流 启动 <task-description>` | Start | bootstrap and route |
| `工作流 探索 <idea>` | Discover | turn vague idea into concrete goal |
| `工作流 对标 <ref-project>` | Benchmark | reference-driven fit-gap |
| `工作流 构思 <topic>` | Ideation | produce candidate approaches |
| `工作流 设计 <task>` | Design | converge upstream output into approved architecture |
| `工作流 工作树 <scope>` | Worktree | prepare isolated workspace/baseline |
| `工作流 调试 <issue>` | Debug | root-cause investigation before fixes |
| `工作流 处理评审反馈 <source>` | Review Feedback | classify and verify feedback changes |
| `工作流 继续` / `workflow continue` | Continue | resume from checkpoint |
| `工作流 状态` / `workflow status` | Status | inspect phase/state |
| `工作流 完成` / `workflow complete` | Complete | close active workflow session |

## Dependency Stack (MUST)

1. `sy-constraints`
2. `sy-workflow-constraints`
3. `sy-code-insight`
4. `sy-ideation`
5. `sy-design`
6. `sy-worktree`
7. `sy-writing-plans`
8. `sy-executing-plans`
9. `sy-debug`
10. `sy-verification-before-completion`
11. `sy-requesting-code-review`
12. `sy-receiving-code-review`
13. `sy-doc-sync`
14. Optional by user request: `sy-changelog`, `sy-git-commit`

## Routing Contract

1. Load `sy-constraints` and `sy-workflow-constraints`.
2. Ensure `sy-code-insight: 初始化` exists; run if missing.
3. Run `sy-code-insight: 分析 <task>`.
4. Route by strongest signal:
   - vague idea -> `探索`
   - reference-driven refactor -> `对标`
   - creative/new/ambiguous requirements -> `构思`
   - approved design missing -> `设计`
   - risky or multi-file implementation prep -> `工作树`
   - approved design + workspace ready -> `计划`
5. Execute with chosen mode (`执行|自动执行|批处理执行|并行执行`) via `sy-executing-plans`.
6. Any repeated verify/review failure MUST route to `调试` via `sy-debug`.
7. Run full verify via `sy-verification-before-completion`.
8. Route to `评审` via `sy-requesting-code-review`.
9. Route review comments to `处理评审反馈` via `sy-receiving-code-review`.
10. When user confirms no further actions in current run, route to `完成`.

If multiple signals conflict and user did not choose explicitly:
- ask one clarification question before routing.

## Session Contract

Canonical state file: `.ai/workflow/session.yaml`
Legacy compatibility: `.ai/workflow/session.md`

Persist and update:
- `run_id`
- `task`
- `current_phase`
- `mode`
- `last_node`
- `next_action`
- `updated_at` (ISO-8601)

Optional workflow artifacts:
- `.ai/workflow/design-decisions.md`
- `.ai/workflow/ledger.md`
- `.ai/workflow/audit.jsonl`
- `.ai/workflow/sprint-status.yaml`

If session state is missing, `工作流 继续` MUST fallback to `工作流 启动`.

## Hooks Alignment

Workflow assumes hook-backed hard guards are active:
- `.claude/settings.json`
- `.claude/sy-hooks.policy.json`
- `scripts/hooks/*`
- `SessionStart` MUST inject bootstrap routing context before the first model turn.
- `Stop` MUST block incomplete checkpoints in active phases.

Hook failures MUST be treated as blocking constraints, not advisory warnings.

## Operations

- **Start**: [operations/start.md](operations/start.md)
- **Discover**: [operations/discover.md](operations/discover.md)
- **Benchmark**: [operations/benchmark.md](operations/benchmark.md)
- **Ideation**: [operations/ideation.md](operations/ideation.md)
- **Review Feedback**: [operations/review-feedback.md](operations/review-feedback.md)
- **Continue**: [operations/continue.md](operations/continue.md)
- **Status**: [operations/status.md](operations/status.md)
- **Complete**: [operations/complete.md](operations/complete.md)

## Guardrails

- MUST apply 1% skill-check rule before any response/action.
- MUST run discover before design for vague intent.
- MUST run benchmark before design for reference-driven refactor.
- MUST run design before plan when architecture is not yet approved.
- MUST disclose workspace/isolation choice before planning risky execution.
- MUST NOT bypass verification/evidence gates.
- MUST route repeated failures to root-cause debugging.
- MUST close finished workflow via `工作流 完成` before ending session.
- MUST NOT commit unless user explicitly requests.
