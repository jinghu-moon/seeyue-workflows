---
name: sy-constraints/execution
description: Use when planning or executing phased work to enforce source-of-truth, phase gates, validation gates, and rollback boundaries.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Execution

## Overview

This skill governs phase-scoped execution, validation discipline, and rollback-safe change boundaries.

## Trigger

Use when:
- entering `plan -> execute -> review` flow
- applying changes across multiple nodes/phases
- touching schema/data/public API contracts

## Iron Rule

```text
NO OUT-OF-PHASE IMPLEMENTATION. NO IRREVERSIBLE CHANGE WITHOUT ROLLBACK PATH.
```

## Protocol

1. Agent MUST treat approved requirements/design/plan as source of truth.
2. Agent MUST execute one phase at a time.
3. Out-of-scope work MUST be marked deferred, not silently implemented.
4. Agent MUST stop after each phase review and ask whether to proceed.

Design gate:
- Agent MUST NOT enter implementation before design approval when requirements are ambiguous.
- If design is missing/unclear, agent MUST route to ideation/plan clarification first.

Node self-audit before completion:

```text
scope       = PASS | FAIL
constraints = PASS | FAIL
truth       = PASS | FAIL
```

Any FAIL MUST block completion.

Hard validation gate (each completed node):
- compile
- type check (when applicable)
- test
- lint
- build
- manual checks (when applicable)

Failed verification MUST trigger bounded auto-fix retries (max 3), then STOP.

## Rollback Boundary (MUST)

Before any phase that modifies schema, data, or public API:
- Agent MUST declare rollback boundary.
- Agent MUST NOT begin the phase without a confirmed restore path.

Required declaration:

```yaml
rollback_boundary:
  revert_nodes:
    - <node-id>
  restore_point: <last-known-good state>
  verification_after_restore:
    - <command>
```

## Commit Safety

Agent MUST NOT perform commit operations unless user explicitly requests commit/release.

## Record Format

```text
ExecutionGate:
  phase: <name>
  source_of_truth: <doc/path>
  in_scope: pass|fail
  validation_gate: pass|fail
  rollback_boundary_declared: pass|fail
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "顺手把下一阶段一起做了" | Phase boundary was broken; stop and split by phase. |
| "先改完再补验证" | Validation lag creates unknown state; verify before claiming node completion. |
| "需求大概明确，可以先做" | Ambiguous requirements require design clarification first. |
| "回滚后面再补" | No rollback path means no safe start for schema/data/public API changes. |

## Red Flags

- "顺手把下一阶段一起做了"
- "先改完再补验证"
- "不需要停下来问，直接继续"
- "先做变更，回滚后面再想"

## When NOT to use

- Pure read-only analysis with no execution action.

## Related Skills

- `sy-constraints`
- `sy-constraints/phase`
- `sy-constraints/verify`
- `sy-constraints/workspace`
