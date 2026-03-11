# Checkpoint & Auto-Fix Rules

## Checkpoint Behavior

Agent MUST pause after each Micro-Node completion. No silent continuation.

Execution modes:

| Mode | Trigger | Checkpoint Behavior |
|------|---------|---------------------|
| Normal | `执行` | STOP after each node, await "CONTINUE" |
| Auto | `自动执行` | No pause, only STOP on verify failure |
| Batch | `批处理执行` | Run max 3 nodes, then STOP for feedback |
| Parallel | `并行执行` | Dispatch by safe group, checkpoint after each group |

| Event | Normal Mode | Auto Mode |
|-------|-------------|-----------|
| Node success | Output progress, STOP, await "CONTINUE" | Output progress, continue to next node |
| Node verify fail | Enter Auto-Fix loop | Enter Auto-Fix loop |
| Auto-Fix exhausted (3 retries) | STOP, report failure details to user | STOP, report failure details to user |
| User says "SKIP" | Mark node as skipped, proceed to next | — |
| User says "ABORT" | Halt entire workflow, output partial summary | — |

| Event | Batch Mode |
|-------|------------|
| Every node success | Continue until 3 nodes or batch end |
| 3 nodes complete | STOP, output batch report, await "CONTINUE" |
| Node fail | Enter Auto-Fix loop; STOP if exhausted |

| Event | Parallel Mode |
|-------|---------------|
| Group preflight pass | Dispatch group concurrently |
| Group preflight fail | Fallback group to sequential, report reason |
| Group verify pass | Output group checkpoint, await "CONTINUE" |
| Group verify fail | Enter Auto-Fix loop; STOP if exhausted |

## Auto-Fix Strategy

Escalating approach — each retry broadens scope.

| Retry | Strategy | Context Change |
|-------|----------|----------------|
| 1 | Root-cause evidence capture (error, stack, failing input) | None |
| 2 | Broaden context — read related files via `理解` | +1 file |
| 3 | Simplify — reduce node scope, defer complex part | Scope shrink |
| 4 | STOP — report to user with full error context | N/A |

## Auto-Fix Constraints

- MUST NOT change node scope without declaring deviation
- MUST NOT retry silently — each retry outputs attempt number and strategy
- IF retry changes approach → declare: "Auto-Fix Retry N: [strategy]"
- Deferred parts from Retry 3 → create new node in plan
- MUST avoid random fix attempts; hypothesis and evidence are required before Retry 2+
- In parallel mode, auto-fix MUST isolate failing node first; group execution pauses until node is resolved

## Failure Report Format

```
❌ Node N2 failed (3/3 retries exhausted)
  Target: src/redirect.rs
  Error: cannot find trait `DryRun` in scope
  Retries:
    1. Fixed import path → same error
    2. Read mod.rs for context → trait not exported
    3. Simplified to direct impl → type mismatch

  Recommendation: Trait definition missing from N1 output.
  Action needed: User review N1 changes or provide guidance.
```

## Evidence Before Claim (MUST)

- No node may be marked complete without fresh verification command output.
- "pass" claims MUST include at least:
  - command
  - exit code
  - key pass signal (e.g., `0 failed`, `build finished`, `0 errors`)

## Session Resume

IF session interrupted (context lost, timeout, crash):

| Step | Action |
|------|--------|
| 1 | User sends `继续` / `CONTINUE` in new session |
| 2 | Agent reads plan (from conversation or .ai/ context) |
| 3 | Identify last completed node — check file state + run verify |
| 4 | Resume from next incomplete node |

Resume output format:

```
🔄 Session resumed from N{X}
  Completed: N1 ✓, N2 ✓
  Remaining: N3, N4
  Progress: 2/4 nodes (50%)

Continuing with N3...
```

Resume constraints:

- MUST re-verify last "completed" node before continuing — file may have been manually edited
- IF verify fails on previously completed node → report conflict, STOP, await user guidance
- MUST NOT re-execute already verified nodes
