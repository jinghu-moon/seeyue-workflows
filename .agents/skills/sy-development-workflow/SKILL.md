---
name: sy-development-workflow
description: Use when legacy prompts reference the old unified development workflow skill and routing must be forwarded to the new split sy-* workflow skills.
allowed-tools:
  - Read
argument-hint: [command] [task]
disable-model-invocation: false
---

# Development Workflow (Compatibility Wrapper)

This skill is kept as a legacy adapter.

## Status

`sy-development-workflow` is deprecated.

Use these split skills instead:
- design -> `sy-design`
- worktree -> `sy-worktree`
- ideation -> `sy-ideation`
- planning -> `sy-writing-plans`
- execute -> `sy-executing-plans`
- debug -> `sy-debug`
- final verify -> `sy-verification-before-completion`
- review -> `sy-requesting-code-review`
- review feedback -> `sy-receiving-code-review`

## Forwarding Contract

When old commands are detected:
- `设计` -> route `sy-design`
- `工作树` -> route `sy-worktree`
- `构思` -> route `sy-ideation`
- `计划` -> route `sy-writing-plans`
- `执行|自动执行|批处理执行|并行执行` -> route `sy-executing-plans`
- repeated verify/review failure -> route `sy-debug`
- `评审` -> route `sy-requesting-code-review`
- `处理评审反馈` -> route `sy-receiving-code-review`
- completion gate -> route `sy-verification-before-completion`

No new logic should be added here.
