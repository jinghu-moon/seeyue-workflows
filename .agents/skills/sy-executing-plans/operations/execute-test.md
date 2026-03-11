# Operation: Execute Test
# Trigger: `执行 测试 <N>`

TDD-only operation for a single node.

## Step 0 - Resolve Node

Read node from plan:
- `tdd_required`
- `red_cmd`
- `green_cmd`
- `risk_level`
- `coverage_threshold` (optional)

If `tdd_required=false`, skip and route to `执行 验证 <N>`.

## Step 1 - RED Gate (MUST FAIL)

Run `red_cmd`.

Valid RED:
- failing because target behavior is not implemented

Invalid RED:
- syntax/import/env/setup failures

If invalid RED, stop and fix test setup first.
If RED unexpectedly passes, stop and ask for clarification.

## Step 2 - GREEN Gate

Run `green_cmd`.

If fail:
- enter auto-fix (max 3 attempts, hypothesis required each attempt)
- only targeted changes

3 attempts exhausted: stop and report failure.

## Step 3 - REFACTOR Gate

Perform minimal cleanup without behavior change, then rerun `green_cmd`.
If fail, revert refactor and stop.

## Step 4 - Coverage Gate

Coverage threshold:
- `critical/core`: 100%
- `standard`: 80%
- `utility`: 60%
- `scaffold`: not enforced

If coverage command configured, run and enforce threshold.

## Step 5 - Anti-Pattern Gate (MUST)

Reject these patterns:
- mock-only behavior assertion
- production method added only for tests
- incomplete mock schema vs real contract

If found, stop and fix tests.

## Step 6 - Persist Evidence

Update `.ai/workflow/session.yaml` (legacy fallback: `.ai/workflow/session.md`):
- `current_node.red_verified: true`
- `updated_at: <ISO-8601>`

Append `.ai/workflow/ledger.md`:
- RED command + exit + failure reason
- GREEN command + exit
- coverage result
- anti-pattern gate result

Output:

```markdown
## Checkpoint: Node <N> Tests

RED: confirmed
GREEN: pass
Coverage: <value/threshold>
Anti-Pattern: pass
Next: 执行 验证 <N>
```
