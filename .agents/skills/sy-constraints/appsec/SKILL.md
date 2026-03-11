---
name: sy-constraints/appsec
description: Use when touching auth, input boundaries, APIs, secrets, or sensitive data so baseline application security controls are enforced.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: AppSec Guardrails

## Overview

This skill applies minimum security controls for authentication, input handling, data protection, and abuse resistance.

## Trigger

Use when tasks touch:
- authentication or authorization
- user input, file upload, or API endpoints
- secrets, credentials, tokens, keys
- payment or sensitive data flows

## Iron Rule

```text
NO SECURITY-CRITICAL CHANGE WITHOUT EXPLICIT AUTH, VALIDATION, AND LEAKAGE CONTROLS.
```

## Protocol

Core rules:
1. Agent MUST NOT hardcode secrets.
2. Agent MUST validate untrusted input at system boundaries.
3. Agent MUST enforce access control before sensitive operations.
4. Agent MUST use injection-safe data access patterns.
5. Agent MUST prevent sensitive-data leakage in logs/errors.
6. Agent MUST include abuse protection where relevant (rate limit/throttling).

Alternative paths for MUST NOT:
- Secrets:
  - use environment variables (`process.env.*`, `.env*`, or platform secret manager)
  - provide template key names when real values are unavailable
- Access control:
  - if full auth cannot be completed now, add explicit deny-by-default guard and mark endpoint non-production
- Logging:
  - redact token/password/key/session fields before output

## Security Checklist

- [ ] No hardcoded secrets in changed files
- [ ] Input validation exists at entry points
- [ ] Authn/Authz checks cover sensitive paths
- [ ] No unsafe query/string interpolation patterns
- [ ] Error/log output redacts sensitive fields
- [ ] Abuse protection present where public endpoint can be spammed

## Incident Rule

If critical security issue is found:
- STOP normal feature flow
- report impact and scope
- prioritize security fix before continuation

## Record Format

```text
SecurityGate:
  auth_control: pass|fail|n/a
  input_validation: pass|fail|n/a
  secret_handling: pass|fail|n/a
  injection_safety: pass|fail|n/a
  logging_redaction: pass|fail|n/a
  abuse_protection: pass|fail|n/a
```

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "先把 token 写死，联调完再改" | Hardcoded secrets are persistent leakage risk; use env/secret manager now. |
| "先不做鉴权，内网环境没事" | Network boundaries drift; apply deny-by-default before exposing functionality. |
| "日志先打全量，后面再脱敏" | Sensitive logs become irreversible exposure once written. |
| "这个接口访问量小，不用限流" | Low initial traffic does not remove abuse risk; enforce basic throttling. |

## Red Flags

- "先把 token 写死，后面再改"
- "先不做鉴权，联调完成后再补"
- "日志里把完整请求都打出来"

## When NOT to use

- Pure local script/read-only tooling with no network/API boundary, no secrets, and no untrusted input.

## Related Skills

- `sy-constraints`
- `sy-constraints/safety`
- `sy-constraints/testing`
- `sy-constraints/verify`
