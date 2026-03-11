---
name: sy-worktree
description: Use when workflow execution requires isolated workspace preparation, baseline checks, and explicit workspace disclosure before planning or implementation.
allowed-tools:
  - Read
  - Bash
  - Glob
  - Grep
argument-hint: [scope]
disable-model-invocation: false
---

# Workspace Preparation

Prepare an isolated workspace or explicitly document why current workspace is used.

## Trigger

Use when:
- user requests `工作树` / `worktree`
- a risky refactor or multi-file change should not start in the current workspace
- the workflow is about to move from design into planning/execution

## When NOT to use

- read-only analysis only
- tiny local docs-only change with no execution risk

## Iron Rule

```text
NO PHASED IMPLEMENTATION WITHOUT A DECLARED WORKSPACE BASELINE.
NO WORKTREE/BRANCH CREATION WITHOUT EXPLICIT USER CONSENT.
```

## Preconditions

1. Load `sy-constraints/workspace`.
2. Check current git baseline:
   - branch name
   - dirty state
   - ignore rules for worktree directory if project-local
3. Decide workspace mode:
   - `current` for safe in-place continuation
   - `worktree` for isolated implementation
   - `sandbox` when only logical isolation is available

## Steps

1. Report current baseline.
2. If isolation is required and git worktree/branch creation would be needed:
   - explain the target path/branch
   - ask for explicit user confirmation before running git operations
3. If user does not approve isolation:
   - continue in current workspace only after explicit risk disclosure
4. Run baseline health checks using project-appropriate commands.
5. Persist `.ai/workflow/session.yaml`:
   - `current_phase: worktree`
   - `next_action: 计划`
   - `workspace_type`
   - `workspace_path`
   - `baseline_status`

## WorkspaceBaseline Record

```text
WorkspaceBaseline:
  workspace_path: <path>
  workspace_type: <branch|worktree|sandbox|current>
  ignore_rules_checked: pass|fail|n/a
  baseline_cmd_1: <command>
  baseline_cmd_1_result: pass|fail|skip
  baseline_cmd_2: <command>
  baseline_cmd_2_result: pass|fail|skip
  baseline_overall: pass|fail
  blockers: <none|details>
```

## Output Template

```markdown
## Workspace Ready

Workspace: <current|worktree|sandbox>
Path: <path>
Baseline: <pass|fail|skip>
Next: `计划`
Risks: <none|details>
```

## Related Skills

- `sy-workflow`
- `sy-writing-plans`
- `sy-constraints/workspace`
- `sy-constraints/execution`