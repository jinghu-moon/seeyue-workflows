---
name: sy-constraints/testing
description: Use when implementing behavior changes or bug fixes so TDD order, anti-pattern gates, coverage baselines, and behavior validation are enforced.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Testing

## Overview

This skill enforces TDD discipline for behavior-changing work and blocks false-green test outcomes.

## Trigger

Use when:
- implementing new behavior
- fixing bugs
- modifying logic that can regress behavior

## Iron Rule

```text
NO PRODUCTION CODE WITHOUT A FAILING TEST FIRST.
Violating the letter of the rules is violating the spirit of the rules.
```

## Protocol

TDD order (MUST):
1. RED (test fails for expected behavior gap)
2. GREEN (minimal code to pass)
3. REFACTOR (keep tests green)

Exception protocol:

```yaml
tdd_exception:
  reason: <concrete reason>
  alternative_verification:
    - cmd: <command>
      covers: <scope>
  user_approved: false
```

Agent MUST NOT proceed until `user_approved=true`.

Anti-pattern gate (MUST block completion):
- mock-only assertion with no behavior validation
- test-only production method
- incomplete mock schema causing false green

## Coverage Baseline

| Risk Level | Coverage Minimum |
|---|---|
| critical | 100% |
| core | 90% |
| standard | 80% |
| utility | 60% |
| scaffold | not enforced |

## Behavior Validation Gate (beyond coverage)

Coverage measures executed lines, not behavior correctness.

For `critical` and `core` nodes, agent MUST additionally verify:
- invalid input rejection paths
- boundary conditions (empty/null/max-size/concurrency)
- adversarial patterns relevant to stack (e.g., injection/path traversal)

Coverage passes but behavior gate fails -> node MUST NOT be marked complete.

## Rationalization Guard

| Excuse | Reality |
|---|---|
| "太简单，不用 RED" | Skip RED = skip TDD. |
| "先写后补测" | Tests-after cannot prove intended behavior. |
| "覆盖率够了就行" | Coverage is necessary, not sufficient. |

## Record Format

```text
TestGate:
  red_observed: yes|no
  green_observed: yes|no
  refactor_green: yes|no
  coverage_actual: <percent>
  coverage_required: <percent>
  behavior_gate: pass|fail
```

## Red Flags

- "先实现，测试后补"
- "这次不走 RED，太简单"
- "覆盖率先不看，能跑就行"

## When NOT to use

- Pure docs/comments/format-only changes with no behavior impact.

## Related Skills

- `sy-constraints`
- `sy-constraints/verify`
- `sy-constraints/debug`
