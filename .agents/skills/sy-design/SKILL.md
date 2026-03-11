---
name: sy-design
description: Use when discovery, benchmark, or ideation outputs must converge into an approved technical design before planning.
allowed-tools:
  - Read
  - WebSearch
argument-hint: [task]
disable-model-invocation: false
---

# Designing

Converge upstream problem framing into approved technical architecture.

## Trigger

Use when:
- user requests `设计` / `design`
- discovery output is actionable but architecture is still not approved
- benchmark output identifies target direction but migration/design choices remain open
- ideation output needs to become an executable technical design

## When NOT to use

- requirements are still too vague to identify components
- implementation is already in progress
- user only wants task breakdown (use `sy-writing-plans` after approval)

## Preconditions

1. Read `.ai/workflow/session.yaml` if present. Legacy fallback: `.ai/workflow/session.md`.
2. Load one upstream source:
   - discovery summary
   - benchmark fit-gap
   - ideation output
   - or direct user description when no upstream artifact exists
3. Confirm whether the project stack detected in `.ai/init-report.md` is still valid.

## Iron Rule

```text
NO PLAN WITHOUT APPROVED DESIGN.
NO DESIGN WITHOUT NON-GOALS AND RISK DISCLOSURE.
```

## Design Contract (MUST)

The design output MUST include:
- `Component Map`
- `Tech Stack Decisions`
- `Data Model`
- `Integration Points`
- `Non-Goals`
- `Top Risks`

## Steps

1. Detect upstream source and restate the problem in one paragraph.
2. Build `Component Map`:
   - component
   - responsibility
   - interface inputs/outputs
   - constraints
3. Record `Tech Stack Decisions`:
   - chosen option
   - alternatives considered
   - reason for choice
4. Summarize `Data Model` and `Integration Points`.
5. Declare explicit `Non-Goals`.
6. List top 3 risks with mitigation.
7. Present design and wait for explicit approval.
8. Only after approval:
   - append `.ai/workflow/design-decisions.md`
   - update `.ai/workflow/session.yaml`
   - set `current_phase: design`
   - set `next_action: 工作流 工作树` for risky/multi-file work, otherwise `计划`

## Output Template

```markdown
## 技术设计

Task: <task>
Source: <discover|benchmark|ideation|direct>

### Component Map
| Component | Responsibility | Interface | Constraints |
|---|---|---|---|
| ... | ... | ... | ... |

### Tech Stack Decisions
| Area | Chosen | Alternative | Reason |
|---|---|---|---|
| ... | ... | ... | ... |

### Data Model
- ...

### Integration Points
- ...

### Non-Goals
- ...

### Top Risks
1. [High|Med|Low] ... → Mitigation: ...
2. ...
3. ...

🔵 是否批准该设计？[yes / revise / scope]
```

## Related Skills

- `sy-workflow`
- `sy-code-insight`
- `sy-writing-plans`
- `sy-constraints/execution`
- `sy-constraints/phase`