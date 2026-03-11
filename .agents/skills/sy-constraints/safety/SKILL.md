---
name: sy-constraints/safety
description: Use when any risky command or sensitive operation is considered, so explicit confirmation and safe alternatives are enforced.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Safety

## Overview

This skill enforces explicit confirmation for high-risk operations and safe handling of sensitive targets.

## Trigger

Use when:
- command can delete/overwrite/history-rewrite
- operation can affect production data/system permissions
- operation may expose sensitive data

## Iron Rule

```text
NO HIGH-RISK OPERATION WITHOUT EXPLICIT CONFIRMATION.
```

## Protocol

1. Before any high-risk action, agent MUST ask explicit confirmation.
2. Agent MUST NOT output or transmit secrets.
3. Agent MUST redact tokens/keys in logs and reports.
4. Agent MUST avoid production endpoints unless user explicitly requires them.

High-risk categories:
- destructive filesystem actions
- git history rewrite or force push
- environment/system permission changes
- production data/schema destructive operations
- sensitive data egress

Command guard (restricted unless explicitly requested):
- `git reset --hard`
- `git push --force`
- destructive delete patterns

Hook-backed runtime guard defaults:
- `git commit` is blocked unless `SY_ALLOW_GIT_COMMIT=1`
- `git push` is blocked unless `SY_ALLOW_GIT_PUSH=1`
- emergency bypass exists via `SY_BYPASS_PRETOOL_BASH=1` and MUST require explicit user confirmation

Required confirmation format (zh-CN OUTPUT):

```text
⚠️ 危险操作检测！
操作类型：[具体操作]
影响范围：[详细说明]
风险评估：[潜在后果]

请确认是否继续？[需要明确的"是"、"确认"、"继续"]
```

Safe alternatives (MUST provide when denying risky command):
- instead of `git reset --hard`: use `git stash push -u` + temporary branch
- instead of force push: create new branch and open review flow
- instead of destructive delete: dry-run list + scoped delete

## Record Format

```text
SafetyCheck:
  operation: <command/action>
  risk_level: low|medium|high
  confirmation_received: yes|no
  safer_alternative_provided: yes|no
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "这个命令应该没风险" | Risk must be assessed explicitly for high-impact commands. |
| "先执行，之后再解释" | High-risk actions require confirmation before execution. |
| "先把 token 打出来看一下" | Sensitive data exposure is an irreversible breach vector. |
| "只是临时 force push 一下" | History rewrite is destructive and requires strict approval path. |

## Red Flags

- "直接执行，之后再解释"
- "这个命令应该没风险"
- "先把 token 打出来看一下"

## When NOT to use

- Pure read operations with no side effects.

## Related Skills

- `sy-constraints`
- `sy-constraints/appsec`
- `sy-constraints/workspace`
