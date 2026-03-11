---
name: sy-workflow-constraints
description: Use when executing sy-* workflow phases to enforce minimal constraint loading, phase gates, and hook-backed hard controls.
allowed-tools:
  - Read
argument-hint: [phase]
disable-model-invocation: false
---

# Workflow Constraints (Adapter)

## Overview

This skill adapts `sy-constraints/*` into concrete workflow gates for plan, execute, and review.

## Trigger

Use when:
- starting workflow execution (`sy-ideation`, `sy-writing-plans`, `sy-executing-plans`, review chain)
- resuming an interrupted workflow session
- validating whether current phase can proceed

## Iron Rule

```text
NO PHASE TRANSITION WITHOUT REQUIRED CONSTRAINTS, VERIFIED EXIT GATE, AND ROLLBACK CONTRACT.
```

## Minimal Loading Contract

Baseline load (MUST):
1. `../sy-constraints/SKILL.md`
2. `../sy-constraints/language/SKILL.md`
3. `../sy-constraints/execution/SKILL.md`

Task-specific load (MUST choose as needed, max +1 by default):
- factual certainty needed -> `../sy-constraints/truth/SKILL.md`
- net-new dependency/utility/design -> `../sy-constraints/research/SKILL.md`
- behavior change or bug fix -> `../sy-constraints/testing/SKILL.md`
- failing checks/unexpected behavior -> `../sy-constraints/debug/SKILL.md`
- review comments -> `../sy-constraints/review/SKILL.md`
- completion/handoff claim -> `../sy-constraints/verify/SKILL.md`
- multi-node/batch/parallel mode -> `../sy-constraints/phase/SKILL.md`
- worktree/sandbox/risky refactor -> `../sy-constraints/workspace/SKILL.md`
- auth/input/API/secret/sensitive data scope -> `../sy-constraints/appsec/SKILL.md`
- risky command/data operation -> `../sy-constraints/safety/SKILL.md`

Escalation load:
- incident/security failure MAY load more than 2 child skills in one pass.

## Phase Boundary Contract

Each phase MUST declare:

```yaml
entry_condition:
  - <precondition>
exit_gate:
  cmd: <verification command>
  pass_signal: <required output string>
  coverage_min: <int>% | n/a
rollback_boundary:
  revert_nodes:
    - <node-id>
  restore_point: <last-known-good state>
```

Rules:
- `entry_condition` MUST NOT be null.
- `exit_gate` failure MUST block phase completion.
- schema/data/public API changes MUST declare `rollback_boundary` before execution.

## Hard Guard Contract (Hooks)

The following controls MUST be enforced by hooks, not skill text only:
- dangerous git/history rewrite block
- sensitive secret write/edit block
- unauthorized commit block
- task completion claim guard (verification evidence required)

Implementation contract:
- hook config: `.claude/settings.json`
- hook scripts: `scripts/hooks/*`

## Node Checklist

- [ ] Current phase scope only
- [ ] Source-of-truth aligned
- [ ] Required constraint skills loaded
- [ ] Reuse decision recorded (if net-new)
- [ ] Verification evidence fresh
- [ ] Checkpoint emitted

## Red Flags

- "先做完再补约束"
- "这一步不用验证"
- "直接继续下个阶段"
- "先修了再查根因"

## When NOT to use

- One-off read-only Q&A not entering workflow phases.

## Related Skills

- `sy-constraints`
- `sy-writing-plans`
- `sy-ideation`
- `sy-executing-plans`
- `sy-verification-before-completion`
- `sy-requesting-code-review`
- `sy-receiving-code-review`
- `sy-workflow`
