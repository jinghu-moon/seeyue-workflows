# Operation: 工作流 对标 <ref-project>

Compare current system against a target reference and produce a design-ready fit-gap result.

## Precondition

- A concrete reference project/system/pattern exists
- No code changes in this operation

## Steps

1. Baseline current system.
2. Profile the reference target.
3. Produce fit-gap matrix:
   - capability
   - current state
   - reference state
   - adopt / adapt / reject
4. Record compliance/risk constraints.
5. Output phased migration strategy.
6. Update `.ai/workflow/session.yaml`:
   - `current_phase: benchmark`
   - `next_action: 工作流 设计 <task>`
7. Route next phase:
   - if comparison still incomplete -> continue `对标`
   - if actionable -> `设计`

## Fit-Gap Template

| Capability | Current | Reference | Decision | Risk |
|---|---|---|---|---|
| ... | ... | ... | adopt/adapt/reject | ... |

## Output Template

```markdown
## Benchmark: <ref-project>

Summary:
- ...

Fit-Gap:
- ...

Migration Strategy:
1. ...
2. ...
3. ...

Next:
- <对标 | 设计>
```