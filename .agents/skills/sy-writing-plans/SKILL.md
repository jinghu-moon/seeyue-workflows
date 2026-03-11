---
name: sy-writing-plans
description: Use when an approved design exists and you need an executable implementation plan before writing code.
allowed-tools:
  - Read
  - Glob
  - Grep
  - WebSearch
argument-hint: [task]
disable-model-invocation: false
---

# Writing Plans

Convert approved design into an execution-ready plan with atomic nodes.

## Trigger

Use when:
- user asks to plan or break down implementation
- design has been clarified and approved
- execution is blocked by missing node-level plan

## When NOT to use

- requirements are still vague (route to `sy-workflow` discover/benchmark/ideation first)
- code is already being edited in current step

## Iron Rule

```text
NO IMPLEMENTATION DURING PLANNING.
NO PLAN WITHOUT PHASE BOUNDARIES AND NODE VERIFICATION CONTRACT.
```

## Preconditions

1. Read `.ai/workflow/session.yaml` if present. Legacy fallback: `.ai/workflow/session.md`.
2. Confirm there is approved design context (design notes / user-confirmed architecture).
3. Load planner prompt:
   - `.agents/skills/sy-code-insight/references/prompts/planner.prompt.md`
4. Run `sy-code-insight` analysis if impact is unclear.

If design is not approved, stop and request design approval first.

## Plan Contract (MUST)

Each node MUST include:
- `id`
- `title`
- `target`
- `action`
- `why`
- `depends_on`
- `verify.cmd`
- `verify.pass_signal`
- `risk_level`
- `tdd_required`

Behavior-change/bug-fix nodes MUST include:
- `red_cmd`
- `green_cmd`

If TDD is infeasible, MUST declare:

```yaml
tdd_exception:
  reason: <concrete reason>
  alternative_verification:
    - cmd: <fallback command>
      covers: <behavior scope>
  user_approved: false
```

## Phase Boundary Contract (MUST)

Each phase MUST declare:

```yaml
entry_condition:
  - <precondition>
exit_gate:
  cmd: <verification command>
  pass_signal: <required output signal>
  coverage_min: <int>% | n/a
rollback_boundary:
  revert_nodes:
    - <node-id>
  restore_point: <last-known-good state>
```

## Decomposition Rules

- Interface/schema nodes before consumer implementation nodes.
- Nodes must be independently verifiable and rollbackable.
- No future-phase scope in current phase nodes.
- Hidden dependencies MUST be explicit in `depends_on`.
- If node count is large, split into independently mergeable phases.

## Parallel Planning Rules

`parallel_group` is allowed only when all are true:
- no file overlap
- no dependency edge in group
- no shared mutable state risk
- same phase
- risk level is not `critical`

## Output Template

```markdown
## Plan: <task>

Phase: <P1/P2/...>
Source of Truth: <doc/path or approved conversation design>

Phase Boundary:
  entry_condition:
    - ...
  exit_gate:
    cmd: ...
    pass_signal: ...
    coverage_min: ...
  rollback_boundary:
    revert_nodes: [...]
    restore_point: ...

### Nodes
| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| N1 | ... | ... | ... | ... | ... |
| N2 | ... | ... | ... | ... | ... |

Awaiting approval. Reply `执行` to proceed.
```

## Guardrails

- MUST NOT write implementation code.
- MUST NOT route to execute without explicit user approval.
- MUST include `why` for every node.
- MUST include rollback boundary when schema/data/public API is touched.
- MUST keep language policy aligned with `sy-constraints/language`.

## Related Skills

- `sy-constraints/execution`
- `sy-constraints/phase`
- `sy-constraints/testing`
- `sy-executing-plans`
