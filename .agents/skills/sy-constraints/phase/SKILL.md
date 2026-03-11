---
name: sy-constraints/phase
description: Use when planning or executing multi-node work to enforce DAG safety, phase boundary contracts, checkpoints, and resumable flow.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Phase Planning

## Overview

This skill governs multi-node execution order, parallel safety, and explicit phase boundary contracts.

## Trigger

Use when:
- plan contains 2+ nodes
- execution may run in batch or parallel
- resume/checkpoint behavior is required

## Iron Rule

```text
NO PHASE EXECUTION WITHOUT EXPLICIT ENTRY, EXIT, AND ROLLBACK CONTRACTS.
```

## Protocol

Node atomicity:
1. Each node MUST have a single clear objective.
2. Each node MUST be independently verifiable.
3. Nodes SHOULD remain small and rollbackable.

Dependency discipline:
- node dependency graph MUST be acyclic.
- shared interface/schema nodes MUST come before consumer nodes.
- hidden dependencies MUST be explicit in `depends_on`.

Parallel safety preflight:
1. DAG check
2. file overlap check
3. shared-state risk check

Any preflight failure MUST fallback to sequential mode.

## Phase Boundary Contract (MUST)

Each phase MUST declare:

```yaml
entry_condition:
  - <precondition>
exit_gate:
  cmd: <verification command>
  pass_signal: <required output string>
  coverage_min: <int>%
rollback_boundary:
  revert_nodes:
    - <node-id>
  restore_point: <state>
```

- Agent MUST NOT declare `entry_condition: null`.
- Agent MUST NOT mark phase complete if `exit_gate` fails.

## Checkpoint & Resume

- After each completed node, agent MUST emit a checkpoint.
- Checkpoint MUST include: node id, verification result, next action.
- Interrupted sessions MUST resume from last verified node.

## Budget Guard (auto/batch/parallel)

Agent SHOULD track and enforce:
- max_nodes
- max_minutes
- max_consecutive_failures

If budget is hit, agent MUST STOP and report.

MUST NOT with alternatives:
- MUST NOT dispatch parallel execution without passing preflight checks.
  - Alternative: fallback to sequential mode and keep explicit checkpoint cadence.
- MUST NOT keep hidden dependencies in node definitions.
  - Alternative: declare `depends_on` explicitly before execution.
- MUST NOT mark phase done when `exit_gate` fails.
  - Alternative: route to debug/fix cycle and re-run exit gate.

## Record Format

```text
PhaseGate:
  phase: <name>
  entry_condition_met: pass|fail
  exit_gate_passed: pass|fail
  rollback_boundary_declared: pass|fail
  mode: sequential|batch|parallel
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "依赖关系先不写，后面再补" | Hidden dependencies break determinism and resume safety. |
| "并行跑一下试试，不做预检" | Preflight is mandatory to avoid collisions and shared-state corruption. |
| "失败了继续下一节点再说" | Failed exit gate blocks phase completion. |
| "先宣告完成，后面补 checkpoint" | Missing checkpoint breaks recovery and auditability. |

## Red Flags

- "依赖关系先不写，后面再补"
- "并行跑一下试试，不做预检"
- "失败了继续下一节点再说"

## When NOT to use

- Single-node atomic tasks with no phase transition.

## Related Skills

- `sy-constraints`
- `sy-constraints/execution`
- `sy-constraints/verify`
