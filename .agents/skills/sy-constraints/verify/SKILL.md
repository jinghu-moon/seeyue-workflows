---
name: sy-constraints/verify
description: Use when preparing any fixed/completed/ready claim so verification evidence is fresh, complete, and auditable.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Verification Matrix

## Overview

This skill defines completion gates and evidence format for trustworthy status claims.

## Trigger

Use when:
- claiming `fixed|passed|completed|ready`
- requesting phase handoff
- closing review iteration

## Iron Rule

```text
NO COMPLETION CLAIMS WITHOUT FRESH VERIFICATION EVIDENCE.
```

## Protocol

Verification matrix (MUST, stack-applicable):
- build
- type check
- lint
- tests
- security scan (minimum practical depth)
- diff review (unintended change scan)

Freshness rule:
- Evidence MUST be produced after the latest relevant code change.
- Prior run history MUST NOT be reused as final proof.

Coverage rule:
- If testing constraints define `coverage_min`, report MUST include:
  - `coverage_actual`
  - `coverage_required`
- `coverage_actual < coverage_required` means `Overall=NOT READY`.

Behavior validation rule (beyond coverage):
- Coverage measures executed lines, not behavior correctness.
- For `critical` and `core` risk nodes, report MUST include behavior checks for:
  - invalid/adversarial input rejection
  - boundary conditions (empty/null/max-size/concurrent)
  - error-path correctness (error type and message)
- Coverage pass + behavior failure means `Overall=NOT READY`.

Alternative path if verification command is unavailable:
- report exact blocker
- provide closest equivalent command and gap
- request user decision before completion claim

## Evidence Format

```text
Command:  <exact command>
Exit:     <exit code>
Signal:   <pass/fail signal from output>
Coverage: <actual>% / <required>% | n/a
```

## Report Format

```text
VERIFICATION REPORT
Build:      PASS|FAIL|N/A
Types:      PASS|FAIL|N/A
Lint:       PASS|FAIL|N/A
Tests:      PASS|FAIL|N/A
Coverage:   <actual>% / <required>% | N/A
Security:   PASS|FAIL|N/A
Diff:       PASS|FAIL
Overall:    READY|NOT READY
```

## Failure Handling

- Any `FAIL` means agent MUST report actual state and MUST NOT claim success.
- Repeated non-converging failures MUST route to `sy-constraints/debug`.

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "这次应该没问题" | "应该" is not verification evidence. |
| "上次跑过就算通过" | Reused history is stale for completion claims. |
| "先说完成，验证晚点补" | Completion claims require evidence before claim. |
| "覆盖率到了就行" | Coverage is necessary, not sufficient for behavior correctness. |

## Red Flags

- "这次应该没问题"
- "上次跑过就算通过"
- "先说完成，验证晚点补"

## When NOT to use

- Pure discovery/ideation conversations with no completion claim.

## Related Skills

- `sy-constraints`
- `sy-constraints/testing`
- `sy-constraints/debug`
- `sy-constraints/execution`
