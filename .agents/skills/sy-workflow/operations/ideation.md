# Operation: 工作流 构思 <topic>

Route ideation requests to `sy-ideation` and then converge into design.

## Steps

1. Validate topic is provided.
2. Update `.ai/workflow/session.yaml`:
   - `current_phase = ideation`
   - `next_action = 工作流 设计 <topic>`
   - refresh `updated_at`
3. Delegate to `sy-ideation`.
4. After explicit design approval need is confirmed:
   - route to `sy-design`

## Output

```markdown
## Workflow Ideation

Topic: <topic>
Delegated: sy-ideation
State: `.ai/workflow/session.yaml` updated
Next: <构思后进入 设计>
```