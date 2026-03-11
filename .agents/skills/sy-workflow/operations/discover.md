# Operation: 工作流 探索 <idea>

Turn a vague idea into an actionable and designable task definition.

## Precondition

- Idea is high-level and lacks clear scope
- No code changes in this operation

## Steps

1. Clarify problem statement (one question per round):
   - what pain/problem should be reduced?
   - who is affected?
   - what must become easier/faster/safer?
2. Identify current workaround and why it is insufficient.
3. Define minimum success signal.
4. Define minimum slice that is worth implementing.
5. Produce discovery output.
6. Update `.ai/workflow/session.yaml`:
   - `current_phase: discover`
   - `next_action: 工作流 设计 <task>`
7. Route next phase:
   - if still unclear -> continue `探索`
   - if actionable -> `设计`

## Output Template

```markdown
## Discover: <idea>

Problem:
- ...

Current Workaround:
- ...

Minimum Success:
- ...

Minimum Slice:
- ...

Next:
- <探索 | 设计>
```