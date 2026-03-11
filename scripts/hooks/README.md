# SY Hook Guards v2

Deterministic enforcement layer for the `sy-*` workflow.

## Architecture

| Layer | Reliability | Purpose |
|---|---:|---|
| Hooks (`scripts/hooks/*.cjs`) | 100% (deterministic) | Hard enforcement at execution boundary |
| `.agents/skills/sy-constraints/*` | Probabilistic trigger | Reasoning protocols and decision discipline |

Rules that MUST always fire belong in hooks, not only in skill text.

## Hook Matrix

| Event | Script | Enforces |
|---|---|---|
| `SessionStart` | `sy-session-start.cjs` | bootstrap routing (`sy-workflow` + `sy-constraints`) |
| `UserPromptSubmit` | `sy-prompt-refresh.cjs` | long-session re-anchor before implementation |
| `BeforeToolSelection` | `sy-before-tool-selection.cjs` | pre-tool routing interception (Gemini-only) |
| `PreToolUse:Bash` | `sy-pretool-bash.cjs` | command class + approval guard, destructive + git safety |
| `PreToolUse:Bash` | `sy-pretool-bash-budget.cjs` | auto/batch/parallel loop budget gate |
| `PreToolUse:Write|Edit` | `sy-pretool-write.cjs` | protected files, TDD red gate, secret gate, debug gate |
| `PreToolUse:Write|Edit` | `sy-pretool-write-session.cjs` | session state integrity for canonical `session.yaml` writes (legacy `session.md` still supported) |
| `PostToolUse:Write|Edit` | `sy-posttool-write.cjs` | audit trail, index invalidation, scope-drift warning |
| `PostToolUse:Bash` | `sy-posttool-bash-verify.cjs` | verification + TDD red/green evidence capture |
| `AfterModel` | `sy-after-model.cjs` | post-model synthesis inspection (Gemini-only) |
| `Stop` | `sy-stop.cjs` | phase-aware completion checkpoint gate |

## Config Files

- `.claude/settings.json`: hook wiring.
- `.claude/sy-hooks.policy.json`: policy overrides.
- `SY_HOOKS_POLICY=<path>`: custom policy path.

## Environment Overrides

- `SY_ALLOW_GIT_COMMIT=1`: allow `git commit` in current shell session.
- `SY_ALLOW_GIT_PUSH=1`: allow `git push` in current shell session.
- `SY_BYPASS_PRETOOL_BASH=1`: bypass bash guard (emergency).
- `SY_BYPASS_PRETOOL_WRITE=1`: bypass write guard (emergency).
- `SY_BYPASS_SECRET_GUARD=1`: bypass secret gate only.
- `SY_BYPASS_STOP_GUARD=1`: bypass stop checkpoint guard.
- `SY_BYPASS_PROMPT_REFRESH=1`: bypass UserPromptSubmit refresh.
- `SY_BYPASS_LOOP_BUDGET=1`: bypass loop budget gate.
- `SY_BYPASS_SESSION_GUARD=1`: bypass session write integrity guard.
- `SY_BYPASS_TOOL_SELECTION=1`: bypass BeforeToolSelection guard.
- `SY_BYPASS_AFTER_MODEL=1`: bypass AfterModel redaction.

## Policy Overrides (example)

`beforeToolSelection` 和 `afterModel` 的推荐配置示例可参考：

- `.claude/sy-hooks.policy.json`

## Quick Smoke Tests

```powershell
'{}' | node scripts/hooks/sy-session-start.cjs
'{"tool_name":"Bash","tool_input":{"command":"git push --force"}}' | node scripts/hooks/sy-pretool-bash.cjs
'{"tool_name":"Write","tool_input":{"file_path":"x.ts","content":"AKIA1234567890ABCDEF"}}' | node scripts/hooks/sy-pretool-write.cjs
'{"tool_name":"Bash","tool_input":{"command":"cargo test"}}' | node scripts/hooks/sy-posttool-bash-verify.cjs
'{}' | node scripts/hooks/sy-stop.cjs
```
