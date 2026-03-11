---
name: sy-constraints/review
description: Use when handling user or reviewer feedback so each item is clarified, classified, implemented, and verified with evidence.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Review Feedback

## Overview

This skill enforces evidence-based review intake and prevents blind "apply all comments" behavior.

## Trigger

Use when:
- receiving user review comments
- processing code-review bot findings
- handling external reviewer suggestions

## Iron Rule

```text
NO EDITS BEFORE FEEDBACK IS CLARIFIED, CLASSIFIED, AND SCOPED.
```

## Protocol

Intake sequence (MUST):
1. Read the full feedback batch first.
2. Clarify ambiguous items before editing.
3. Classify each item as one of:
   - `accept`
   - `reject`
   - `unverified`
   - `defer`
4. Implement only `accept` items one by one.
5. Verify each implemented item before moving to next.
6. Report unresolved `reject|unverified|defer` items explicitly.

Classification rule:
- `reject` MUST include technical rationale and evidence.
- `unverified` MUST include required proof command/source.
- `defer` MUST include phase/scope boundary.

Pushback rule:
- Agent MAY push back, but MUST provide technical evidence.
- If feedback conflicts with approved source-of-truth, agent MUST request user priority decision.

Forbidden behavior (MUST NOT):
- performative agreement without validation
- blind implementation of all comments
- silently skipping unclear items

Alternative path if item is unclear:
- do not edit
- ask one focused clarification question
- continue only after explicit clarification

## Record Format

```text
ReviewDecision:
  item: <feedback item>
  class: <accept|reject|unverified|defer>
  reason: <technical reason>
  action: <change made or follow-up needed>
  evidence: <command/output/path:line>
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "都改了再说，后面统一验证" | Batch blind edits hide failures; classify and verify item by item. |
| "不太懂但先按评论改" | Unclear items must be clarified before any edit. |
| "这个评论看着像建议，先忽略" | Unresolved items must be explicitly marked `reject|unverified|defer`. |
| "先回复同意，代码后补" | Performative agreement without evidence violates review protocol. |

## Red Flags

- "都改了再说"
- "不太懂但先按评论改"
- "这个看起来像建议，先忽略"

## When NOT to use

- No review feedback exists and no reviewer-sourced change request is present.

## Related Skills

- `sy-constraints`
- `sy-constraints/truth`
- `sy-constraints/verify`
- `sy-constraints/execution`
