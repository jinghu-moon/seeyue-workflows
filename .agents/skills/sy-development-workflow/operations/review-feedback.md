# Operation: Review Feedback <feedback-source>

Process code review feedback with verify-first discipline.

## Precondition

- Feedback available (PR comments / inline review / user list)
- Trigger: `处理评审反馈 <feedback-source>`

## Steps

1. Normalize feedback items
   - split into atomic items
   - assign id: `F1`, `F2`, ...
2. Clarify before coding
   - if any item unclear, ask clarification first
   - MUST NOT implement partial subset while critical items are unclear
3. Verify each item against codebase reality
   - classify:
     - `accept` (valid and actionable)
     - `reject` (technically incorrect / violates constraints)
     - `unverified` (insufficient evidence)
     - `defer` (out of scope / phase-gated)
4. Implement accepted items one-by-one
   - apply minimal change
   - run targeted verification after each item
5. Report rejected/unverified items
   - rejected: include technical rationale and evidence
   - unverified: include missing evidence and proposed next check
   - deferred: include scope/phase reason and follow-up phase
6. Final validation
   - run required aggregate checks (compile/test/lint/build as applicable)
7. Output summary
   - resolved items, remaining items, and next action

## Rules

- MUST prioritize correctness over social agreement
- MUST NOT use performative acknowledgements as acceptance criteria
- MUST keep item-level audit trail (`F# -> decision -> evidence -> status`)
- MUST preserve phase scope and constraints while applying feedback
- MUST NOT silently skip `defer` items; they must be reported explicitly

## Output Template

```markdown
## Review Feedback Processing

Source: <feedback-source>
Items: <N>

Accepted:
- F1: <summary> | Evidence: <file:line/command> | Status: fixed

Rejected:
- F2: <summary> | Rationale: <technical reason> | Evidence: <...>

Unverified:
- F3: <summary> | Missing: <what evidence is required>

Deferred:
- F4: <summary> | Reason: <scope/phase boundary> | Follow-up: <phase>

Validation:
- compile: <pass/fail/skip>
- test: <pass/fail/skip>
- lint: <pass/fail/skip>
- build: <pass/fail/skip>

Next:
- <继续处理反馈 | 返回评审 | 进入下一阶段>
```
