# Operation: 工作流 完成

Gracefully close the active workflow session and mark it as completed.

## Precondition

- `.ai/workflow/session.yaml` exists, or legacy `.ai/workflow/session.md` exists
- If missing -> report no active workflow and suggest `工作流 启动 <task-description>` for new work

## Steps

1. Read workflow session state.
2. Optionally read latest `.ai/analysis/ai.report.json`.
3. Update session state to completed:
   - `current_phase = done`
   - `next_action = wait new task`
   - keep existing `run_id/task/mode/last_node`
   - refresh `updated_at`
4. Write back `.ai/workflow/session.yaml`
5. Output completion summary and next expected command

## Completion State Template

```yaml
schema: 2
run_id: wf-20260308-001
task: <task-description>
current_phase: done
mode: normal
last_node: N7
next_action: wait new task
updated_at: 2026-03-08T00:00:00Z
```

## Output

```markdown
## Workflow Completed

Task: <task-description>
Phase: done
Next: `工作流 启动 <task-description>`
State Updated: `.ai/workflow/session.yaml`
```