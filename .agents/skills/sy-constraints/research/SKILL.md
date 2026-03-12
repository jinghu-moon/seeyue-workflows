---
name: sy-constraints/research
description: Use when deciding whether to adopt, extend, or build a solution before writing net-new code or adding dependencies.
allowed-tools:
  - Read
  - WebSearch
argument-hint: "[context]"
disable-model-invocation: false
---

# Constraints: Research & Reuse First

## Overview

This skill enforces search-before-build and structured reuse decisions.

## Trigger

Use when:
- introducing a new utility/helper/abstraction
- selecting a dependency or integration
- implementing common patterns likely solved elsewhere

## Iron Rule

```text
SEARCH BEFORE BUILD. DECIDE ADOPT/EXTEND/BUILD WITH EVIDENCE.
```

## Protocol

1. Agent MUST search local repository reuse first.
2. Agent MUST search official docs and package registries as applicable.
3. Agent MUST evaluate at least one viable candidate before net-new build.
4. Agent MUST produce an `adopt|extend|build` decision with rationale.
5. Agent SHOULD prefer battle-tested solutions that satisfy requirements.
6. Agent MUST record maintenance/license/security considerations for adopted dependencies.

## Record Format

```text
ResearchDecision:
  need: <problem>
  query: <search query>
  candidate: <name/link>
  fit: high|medium|low
  decision: adopt|extend|build
  reason: <why>
  checked: <YYYY-MM-DD>
```

## Stop Condition

If no candidate is sufficiently verifiable:
- mark `[unverified]`
- explain risk
- request user decision before implementation

MUST NOT with alternatives:
- MUST NOT start net-new build before at least one viable candidate is evaluated.
  - Alternative: record `candidate=none` with explicit search evidence and proceed only after user approval.
- MUST NOT adopt dependency without maintenance/license/security notes.
  - Alternative: defer adoption and use local extension with explicit risk note.

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "先写出来再搜" | Search-later causes duplicate logic and expensive rework. |
| "应该有库，但先不找了" | Assumptions are not evidence; run repository/docs search first. |
| "这个功能我直接重写更快" | Local speed can create long-term maintenance debt; compare options first. |
| "许可证后面再看" | License/security checks are adoption prerequisites, not follow-up tasks. |

## Red Flags

- "先写出来再搜"
- "应该有库，但先不找了"
- "这个重复功能我直接重写更快"

## When NOT to use

- typo fixes, formatting-only changes, comment updates, or clearly bounded local bugfix with known root cause.

## Related Skills

- `sy-constraints`
- `sy-constraints/truth`
- `sy-constraints/execution`
