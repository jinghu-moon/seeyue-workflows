# Operation: Ideation <topic>

Convert idea/request into an approved design before planning/implementation.

## Precondition

- Triggered by `构思 <topic>` OR implicitly required by constraint gate
- No code changes in this phase

## Hard Gate (MUST)

- MUST NOT enter `计划` / `执行` until design is explicitly approved by user
- If requirement is unclear, ask exactly one clarification question per round

## Steps

1. Explore context:
   - inspect relevant modules/docs/constraints
   - check whether similar capability already exists (YAGNI/DRY)
2. Clarify requirements:
   - ask focused questions one-by-one (goal, constraints, success criteria)
   - if reference-driven refactor, ask fit-gap questions first:
     - what to keep
     - what to borrow
     - what to replace
     - what to drop
3. Propose options:
   - provide 2-3 approaches with trade-offs and one recommendation
4. Present design:
   - architecture, module boundaries, data flow, error handling, test strategy
   - call out in-scope/out-of-scope explicitly
5. Ask for explicit approval:
   - only after approval can workflow enter `计划`
6. Persist design draft:
   - SHOULD save under `docs/plans/<date>-<topic>-design.md` when repo policy allows
7. Reference-driven notes (when applicable):
   - record license/IP boundary assumption
   - record migration rollback boundary per phase

## Output Template

```markdown
## Ideation: <topic>

### Goal
- ...

### Options
1. A: ...
2. B: ...
3. C: ...

### Recommended
- ...
- Why: ...

### Proposed Design
- Architecture: ...
- Data Flow: ...
- Error Handling: ...
- Testing Strategy: ...
- In Scope: ...
- Out of Scope: ...

Please confirm this design. Reply `确认设计` to continue to `计划`.
```
