---
name: sy-constraints/truth
description: Use when making factual or technical claims that require citation, freshness checks, and evidence-before-claim discipline.
allowed-tools:
  - Read
  - WebSearch
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Truth & Evidence

## Overview

This skill prevents fabrication and enforces verifiable claims with explicit evidence.

## Trigger

Use when:
- claiming API behavior, version, benchmark, status, or completion
- handling time-sensitive facts
- citing docs, source files, or external references

## Iron Rule

```text
NO FACTUAL CLAIM WITHOUT VERIFIABLE EVIDENCE.
```

## Protocol

1. Agent MUST NOT invent APIs, flags, file paths, versions, behavior, or metrics.
2. If uncertain, agent MUST mark the item as `[unverified]` before proceeding.
3. Time-sensitive facts MUST be verified with fresh search, not memory.
4. Before claiming fixed/passed/completed, agent MUST provide command evidence.

Accepted sources:
- official documentation URL
- repository link (prefer immutable ref)
- current-session web search results
- local file path with line reference

## Record Format

```text
Claim:    <assertion>
Source:   <url or path:line>
Checked:  <YYYY-MM-DD>
Status:   verified | unverified
```

Completion evidence format:

```text
Command: <exact command>
Exit:    <exit code>
Signal:  <pass/fail signal from output>
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "这个 API 应该是这样" | "应该" is not evidence; provide source or mark `[unverified]`. |
| "大概没问题，可以继续" | Probabilistic confidence is not a factual claim. |
| "上次跑过，这次就不跑了" | Time-sensitive and completion claims require fresh evidence. |
| "这个版本号我记得" | Memory is fallible; verify against current authoritative source. |

## Red Flags

- "这个 API 应该是这样"
- "大概没问题，可以继续"
- "上次跑过，这次就不跑了"

## When NOT to use

- Pure preference discussions with no factual assertion.

## Related Skills

- `sy-constraints`
- `sy-constraints/verify`
- `sy-constraints/research`
