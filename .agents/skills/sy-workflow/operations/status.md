# Operation: 工作流 状态

Show current workflow progress and actionable next step.

## Steps

1. Read `.ai/workflow/session.yaml` (legacy fallback: `.ai/workflow/session.md`).
2. Read `.ai/analysis/ai.report.json` if it exists.
3. Summarize:
   - task
   - current phase
   - mode
   - last node
   - latest report timestamp
   - next action
   - lifecycle (`active` or `completed`)
4. Validate minimal consistency:
   - if session `last_node` conflicts with report `run.node_id`, flag warning

## Output

```markdown
## Workflow Status

Task: <...>
Phase: <...>
Mode: <normal|auto|batch|parallel>
Last Node: <...>
Report: `.ai/analysis/ai.report.json` @ <timestamp or missing>
Next: <exact command>
Lifecycle: <active|completed>
Warnings: <none | mismatch details>
```