---
name: sy-constraints/workspace
description: Use when setting branch/worktree/sandbox context so baseline health and isolation are validated before execution.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Workspace Isolation

## Overview

This skill prevents high-risk changes from running in unknown or dirty environments.

## Trigger

Use when:
- creating or switching branch/worktree
- entering sandbox-isolated execution
- running risky refactor or broad batch/parallel edits

## Iron Rule

```text
NO HIGH-RISK EXECUTION WITHOUT A VERIFIED BASELINE IN A KNOWN WORKSPACE.
```

## Protocol

1. Agent SHOULD use isolated workspace for risky or wide-scope work.
2. Agent MUST verify ignore rules before creating or using project-local worktrees.
3. Agent MUST run baseline health checks before feature execution:
   - dependency sync/install (if required)
   - minimal compile/build/test baseline
4. If baseline fails, agent MUST report existing failures before new edits.
5. For schema/data/public API changes, workspace record MUST reference rollback boundary owner node.

Alternative path if isolation cannot be created:
- continue in current workspace only after explicit risk disclosure
- narrow change scope to smallest safe unit
- increase checkpoint frequency

## Record Format

```text
WorkspaceBaseline:
  workspace_path: <path>
  workspace_type: <branch|worktree|sandbox|current>
  ignore_rules_checked: pass|fail
  baseline_cmd_1: <command>
  baseline_cmd_1_result: pass|fail
  baseline_cmd_2: <command>
  baseline_cmd_2_result: pass|fail
  baseline_overall: pass|fail
  blockers: <none|details>
  rollback_boundary_ref: <phase/node or n/a>
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "先在当前脏环境直接改" | Dirty baseline hides regressions; isolate first or disclose risk explicitly. |
| "基线失败先不管，后面再说" | Existing failures invalidate new evidence; report baseline failures first. |
| "ignore 规则晚点检查" | Missing ignore checks can pull generated/secret artifacts into scope. |
| "不建隔离环境也能并行改" | Parallel edits without baseline isolation increase conflict and rollback risk. |

## Red Flags

- "先在当前脏环境直接改"
- "基线失败先不管，后面再说"
- "不确认 ignore，先建 worktree"

## When NOT to use

- Tiny read-only inspection with no workspace mutation and no code execution.

## Related Skills

- `sy-constraints`
- `sy-constraints/execution`
- `sy-constraints/phase`
- `sy-constraints/safety`
