# Workflow Session Schema

## Canonical File

- `.ai/workflow/session.yaml`
- legacy fallback：`.ai/workflow/session.md`（仅兼容读取，不作为规范输出）

## Required Top-Level Fields

- `schema`（固定为 `4`）
- `run_id`
- `engine`
- `task`
- `phase`
- `node`
- `loop_budget`
- `context_budget`
- `workspace`
- `approvals`
- `recovery`
- `timestamps`

## Key Subfields (摘录)

### engine
- `kind`：`claude_code | codex | gemini_cli`
- `adapter_version`：正整数

### task
- `id`
- `title`
- `mode`：`feature | bugfix | refactor | docs | research`

### phase
- `current`
- `status`：`pending | in_progress | blocked | review | completed`

### node
- `active_id`
- `state`：`idle | red_pending | red_verified | green_pending | green_verified | refactor_pending | verified | failed`
- `owner_persona`：`planner | author | spec_reviewer | quality_reviewer | reader | auditor | human`

### loop_budget
- `max_nodes`
- `max_failures`
- `max_pending_approvals`
- `consumed_nodes`
- `consumed_failures`

### approvals
- `pending`
- `pending_count`
- `last_grant_scope`
- `last_approval_mode`
- `active_request`
- `grants`

### recovery
- `last_checkpoint_id`
- `restore_pending`
- `restore_reason`

### timestamps
- `created_at`
- `updated_at`

## Minimal Example

```yaml
schema: 4
run_id: wf-20260307-001
engine:
  kind: codex
  adapter_version: 1
task:
  id: v4-implementation-plan
  title: Execute implementation-plan-v4 through P7-N4
  mode: feature
phase:
  current: P3
  status: in_progress
node:
  active_id: P3-N1
  state: red_pending
  owner_persona: author
loop_budget:
  max_nodes: 30
  max_failures: 2
  max_pending_approvals: 2
  consumed_nodes: 5
  consumed_failures: 0
context_budget:
  strategy: hybrid
  capsule_refresh_threshold: 4
  summary_required_after_turns: 8
workspace:
  root: D:/100_Projects/110_Daily/VibeCast/seeyue-workflows
  sandbox_mode: full_access
approvals:
  pending: false
  pending_count: 0
  last_grant_scope: none
  last_approval_mode: none
  active_request: null
  grants: []
recovery:
  last_checkpoint_id: null
  restore_pending: false
  restore_reason: null
timestamps:
  created_at: "2026-03-07T08:00:00Z"
  updated_at: "2026-03-07T16:00:00Z"
```

## Compatibility Rules

- 新写入与示例必须以 `session.yaml`（schema=4）为准。
- legacy flat 字段（如 `current_phase`、`next_action`、`last_completed_node`）仅用于兼容读取，不再是规范字段。
