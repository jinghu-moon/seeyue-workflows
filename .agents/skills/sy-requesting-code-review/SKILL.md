---
name: sy-requesting-code-review
description: Use when verification is READY and a structured evidence-first code review must produce a PASS, CONCERNS, REWORK, or FAIL verdict.
allowed-tools:
  - Read
  - Bash
  - Glob
  - Grep
argument-hint: [scope]
disable-model-invocation: false
---

# Requesting Code Review

Run structured code review after full verification passes.

## Trigger

Use when:
- user requests `评审` / `review`
- verification report exists and indicates ready state
- execution phase transitions to review

## Precondition

1. `.ai/workflow/session.yaml` exists and phase is `review`. Legacy fallback: `.ai/workflow/session.md`.
2. `.ai/analysis/ai.report.json` exists and `overall=READY`.

If unmet, stop and route to `sy-verification-before-completion`.

## Reviewer Persona

Load:
- `references/personas/reviewer.md`

Apply evidence-first review with severity ordering.

## Review Flow

1. collect full evidence:
   - changed files (`git diff`)
   - verification report
   - per-node verify commands quick re-run
   - ledger trace
2. evaluate:
   - spec compliance
   - correctness/regression
   - testing/TDD evidence
   - security/safety
   - scope integrity
3. classify findings:
   - Critical / High / Medium / Low
4. output verdict:
   - `PASS`
   - `CONCERNS`
   - `REWORK`
   - `FAIL`

## Verdict Routing

- `PASS` -> `current_phase: done`, `next_action: 提交`
- `CONCERNS` -> `current_phase: done`, `next_action: 提交` (concerns logged)
- `REWORK` -> `current_phase: execute`, `next_action: 处理评审反馈`
- `FAIL` -> block and wait user intervention

## Output Contract

Findings must be listed first, ordered by severity, each with:
- evidence
- impact
- actionable fix

Then output verification summary and next action.

## Related Skills

- `sy-verification-before-completion`
- `sy-receiving-code-review`
- `sy-constraints/review`
