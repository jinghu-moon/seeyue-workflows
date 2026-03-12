---
name: sy-receiving-code-review
description: Use when review feedback must be processed item-by-item with verify-first discipline, classification, and per-item evidence before proceeding.
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
  - Glob
  - Grep
argument-hint: "[source]"
disable-model-invocation: false
---

# Receiving Code Review

Process review feedback as verifiable atomic items.

## Trigger

Use when:
- user requests `处理评审反馈 <source>`
- review verdict is `REWORK` or reviewer comments need handling

## Step 0 - Source Gate

Accept sources:
- inline text
- local file path
- pasted review thread

If source unavailable, ask user to provide concrete feedback text/path.

## Step 1 - Normalize Feedback

Split feedback into atomic items:
- F1, F2, F3...
- one behavior claim per item
- keep original statement

## Step 2 - Clarification Gate

If any item ambiguous:
- ask clarification before implementing any item
- do not continue until ambiguity resolved

## Step 3 - Item Loop (verify-first)

For each item:
1. read relevant code
2. classify: `accept | reject | unverified | defer`
3. if `accept`:
   - implement minimal change
   - run targeted verify
   - record evidence
4. if `reject`:
   - record technical rationale + evidence
5. if `unverified`:
   - state missing evidence
6. if `defer`:
   - log deferred reason to ledger

Checkpoint after each fixed item.

## Step 4 - Aggregate Verification

After item loop:
- run compile/type-check/lint/test/build (as applicable)
- if fail, route to debug/fix
- if pass, route back to review

## Step 5 - Output Report

Report sections:
- accepted/fixed
- rejected
- unverified
- deferred
- aggregate verification status
- next action

## Session Routing

- unresolved high-risk unverified items -> stay in review-feedback
- all fixed + aggregate pass -> `current_phase: review`, `next_action: 评审`
- aggregate fail -> `current_phase: execute`, `next_action: 调试`

## Related Skills

- `sy-requesting-code-review`
- `sy-executing-plans`
- `sy-constraints/review`
