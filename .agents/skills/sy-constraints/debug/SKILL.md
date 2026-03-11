---
name: sy-constraints/debug
description: Use when a bug, failing check, or unexpected behavior appears and a fix is being considered.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Root-Cause Debugging

## Overview

This skill prevents guess-fix loops and enforces evidence-first root-cause debugging.

## Trigger

Use when:
- tests/build/lint/type checks fail
- runtime behavior deviates from expected behavior
- repeated auto-fix attempts do not converge

## Iron Rule

```text
NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST.
Violating the letter of this process is violating the spirit of debugging.
```

## Protocol

1. Capture evidence (error output, exit code, repro steps).
2. Reproduce consistently and define expected vs actual behavior.
3. Check recent changes and component boundaries.
4. Form ONE hypothesis at a time.
5. Run minimal experiment to confirm/falsify.
6. Apply a single fix for confirmed root cause.
7. Re-run relevant verification matrix.

Escalation:
- If 3 fix attempts fail, agent MUST STOP and request architectural decision.
- Agent MUST NOT attempt a 4th blind fix.

## Rationalization Guard

| Excuse | Reality |
|---|---|
| "先改一把试试" | That is guess-fix, not debugging. |
| "多改几处一起看" | Multi-change hides causality. |
| "先绕过去再说" | Symptom bypass preserves root cause. |

## Record Format

```text
DebugRecord:
  issue: <symptom>
  evidence: <command/output/path:line>
  hypothesis: <single cause>
  experiment: <minimal test>
  result: confirmed|falsified
  decision: fix|escalate
```

## Red Flags

- "先改一把试试"
- "我感觉是这里的问题"
- "多改几处一起看看能不能过"
- "先绕过去，后面再查根因"

## When NOT to use

- Pure feature implementation where no failure signal exists.

## Related Skills

- `sy-constraints`
- `sy-constraints/testing`
- `sy-constraints/verify`
