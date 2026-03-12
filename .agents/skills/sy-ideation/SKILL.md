---
name: sy-ideation
description: Use when requirements are creative, ambiguous, or not yet decision-ready and you need structured ideation to produce an approvable design before planning.
allowed-tools:
  - Read
  - Glob
  - Grep
  - WebSearch
argument-hint: "[topic]"
disable-model-invocation: false
---

# Ideation

Convert unclear requests into an approvable design artifact.

## Trigger

Use when:
- user asks `构思` / `设计思路` / `方案构思` / ideation / brainstorming / brainstorm / ideate / concept
- requirements are ambiguous or have major trade-offs
- workflow detects missing design approval before planning

## When NOT to use

- design is already approved and implementation plan is required
- task is pure bug-fix with clear root cause and no design decision

## Iron Rule

```text
NO PLAN OR IMPLEMENTATION BEFORE EXPLICIT DESIGN APPROVAL.
```

## Ideation Protocol

1. Clarify goal and success signal (one focused question per round).
2. Identify constraints:
   - technical
   - product/scope
   - security/compliance
3. Propose 2-3 options with trade-offs.
4. Give one recommended option and rationale.
5. Produce design snapshot:
   - architecture
   - boundaries/interfaces
   - data flow
   - error strategy
   - test strategy
   - in-scope / out-of-scope
6. Ask for explicit approval:
   - user must reply `确认设计` before entering `sy-writing-plans`.

## Output Template

```markdown
## Ideation: <topic>

Goal:
- ...

Constraints:
- ...

Options:
1. A: ...
2. B: ...
3. C: ...

Recommendation:
- ...
- Why: ...

Proposed Design:
- Architecture: ...
- Interfaces: ...
- Data Flow: ...
- Error Strategy: ...
- Test Strategy: ...
- In Scope: ...
- Out of Scope: ...

Please confirm this design. Reply `确认设计` to continue to `计划`.
```

## References

- [references/question-templates.md](references/question-templates.md)

## Related Skills

- `sy-workflow`
- `sy-writing-plans`
- `sy-constraints/phase`
- `sy-constraints/execution`
