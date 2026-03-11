# Workflow Session Schema

## Canonical File

- `.ai/workflow/session.yaml`
- legacy fallback：`.ai/workflow/session.md`

## Required Fields

- `run_id`
- `task`
- `current_phase`
- `mode`
- `last_completed_node`
- `next_action`
- `updated_at`

## Common Phases

- `exploring`
- `benchmarking`
- `free-ideation`
- `designing`
- `plan`
- `execute`
- `review`
- `done`

## Minimal Example

```yaml
run_id: wf-20260307-001
task: extract workflow into standalone repository
current_phase: execute
mode: normal
last_completed_node: N2
next_action: /execute verify N3
updated_at: 2026-03-07T16:00:00Z
```

## Compatibility Rules

- 所有新文档、新示例、新写入都应以 `session.yaml` 为主
- hooks 读取顺序：先 `session.yaml`，后 `session.md`
- `session.md` 仅作为 legacy compatibility 保留
