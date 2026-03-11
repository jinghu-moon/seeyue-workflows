# Checkpoint Rules

## Mode Behavior

| Mode | Pause Policy |
|---|---|
| normal | pause after each verified node |
| auto | continue until failure or budget hit |
| batch | pause every 3 verified nodes |
| parallel | pause after each verified group |

## Evidence Before Completion

A verified node MUST include:
1. command
2. exit code
3. pass signal

No "should pass" statements are accepted as evidence.

## Auto-Fix Escalation

Attempt 1:
- capture root-cause evidence

Attempt 2:
- broaden context and apply focused fix

Attempt 3:
- narrow scope and propose deferred node if needed

After 3 failures:
- stop
- emit failure report
- request explicit user decision

## Session Resume

On `继续`:
1. read plan + session
2. re-verify `last_completed_node`
3. if pass -> continue with next incomplete node
4. if fail -> stop and report state mismatch
