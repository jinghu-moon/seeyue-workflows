# Operation: 工作流 处理评审反馈 <source>

Route review feedback handling to `sy-receiving-code-review` with session state update.

## Steps

1. Validate source is available (text/path/PR comments)
2. Update workflow session:
   - `current_phase = review_feedback`
   - `next_action = 处理评审反馈 <source>`
   - refresh `updated_at`
3. Delegate to:
   - `sy-receiving-code-review`
4. After feedback processing:
   - update `next_action` to `评审` or `验证` per result

## Output

```markdown
## Workflow Review Feedback

Source: <source>
Delegated: sy-receiving-code-review
State: `.ai/workflow/session.yaml` updated (legacy fallback: `.ai/workflow/session.md`)
Next: <评审 | 验证 | 下一阶段>
```
