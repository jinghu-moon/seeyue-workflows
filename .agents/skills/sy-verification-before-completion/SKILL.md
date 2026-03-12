---
name: sy-verification-before-completion
description: Use when all planned nodes are complete and a full session-level verification gate must run before review or completion claims.
allowed-tools:
  - Read
  - Bash
  - Glob
  - Grep
argument-hint: "[scope]"
disable-model-invocation: false
---

# Verification Before Completion

Session-level verification gate executed after node-level verify passes.

## Trigger

Use when:
- user requests `验证` / `verify`
- all nodes are complete and workflow should transition to review
- final completion claim is being prepared

## When NOT to use

- not all plan nodes are complete
- no approved plan context exists
- user only needs single-node verify (use `sy-executing-plans` verify op)

## Iron Rule

```text
NO COMPLETION CLAIM WITHOUT FRESH FULL-RUN VERIFICATION EVIDENCE.
```

## Precondition Gate

1. Read `.ai/workflow/session.yaml` first. Legacy fallback: `.ai/workflow/session.md`.
2. Confirm phase is execution tail (`execute` or `verify`).
3. Confirm all plan nodes are complete.
4. Confirm per-node ledger evidence exists.

If any gate fails, stop and output required next command.

## Verification Phases (fixed order)

1. build
2. type-check
3. lint
4. tests + coverage
5. security/log audit
6. intent-delta verification

Intent-delta means verifying claimed behavior change is observable, not just command exit 0.

## Scope Audit

Cross-check changeset against plan node targets:
- unexpected files -> warning
- unresolved secret findings -> fail

## Report Output (MUST)

Write `.ai/analysis/ai.report.json` with:
- verification summary
- coverage actual vs floor
- security findings
- intent-delta status
- scope warnings
- overall: `READY|NOT_READY`

## Session Update

If overall is `READY`:
- `current_phase: review`
- `next_action: 评审`

If overall is `NOT_READY`:
- keep in execute/verify
- route to debug/fix then rerun verify

## Related Skills

- `sy-executing-plans`
- `sy-requesting-code-review`
- `sy-constraints/verify`
