---
name: sy-debug
description: Use when verify, review, build, test, or runtime failures require structured root-cause debugging before any fix is applied.
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
  - Glob
  - Grep
argument-hint: [issue]
disable-model-invocation: false
---

# Systematic Debugging

Run a root-cause workflow before applying fixes.

## Trigger

Use when:
- build/test/lint/type-check fails
- `execute-verify` enters repeated auto-fix attempts
- review exposes a correctness defect
- runtime behavior differs from expected behavior

## When NOT to use

- feature work with no failure signal
- pure planning or design discussion

## Iron Rule

```text
NO FIXES WITHOUT ROOT-CAUSE INVESTIGATION FIRST.
WRITES ARE BLOCKED UNTIL DEBUG PHASE 5.
```

## Session Contract

Persist to `.ai/workflow/session.yaml`:
- `debug_active: true`
- `debug_phase: 1|2|3|4|5`
- `debug_hypothesis_n: <count>`
- `debug_node_id: <node-id or bug-id>`

Hooks read these fields and MUST block source writes before phase 5.

## Phases

1. **Capture Evidence**
   - collect exact command, exit code, stderr/stdout, repro steps
2. **Reproduce and Define**
   - expected vs actual
   - consistency check
3. **Isolate**
   - boundary tracing
   - recent changes
   - minimal failing unit
4. **Hypothesis and Experiment**
   - form ONE hypothesis
   - run one minimal experiment
   - increment `debug_hypothesis_n`
5. **Fix and Verify**
   - apply one focused fix for confirmed root cause
   - rerun targeted verification first
   - rerun broader verification before exit

## Escalation

- If 3 hypotheses/fixes fail, STOP and request architectural decision.
- Do not attempt a 4th blind fix.

## Output Template

```markdown
## Debug Record

Issue: <symptom>
Node: <node-id or n/a>
Phase: <1..5>
Evidence:
- <command + result>

Hypothesis:
- <single cause>

Experiment:
- <minimal test>

Result:
- confirmed | falsified

Next:
- <continue debugging | apply fix | escalate>
```

## Related Skills

- `sy-workflow`
- `sy-executing-plans`
- `sy-verification-before-completion`
- `sy-constraints/debug`
- `sy-constraints/testing`