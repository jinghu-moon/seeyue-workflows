# Operation: Execute Verify
# Trigger: `执行 验证 <N>`

Node-level verification and checkpoint.

## Step 0 - Resolve Node

Required fields:
- `target`
- `verify.cmd`
- `tdd_required`

## Step 1 - TDD Evidence Pre-Gate

If `tdd_required=true`, require:
- RED evidence exists
- GREEN evidence exists

If missing: block verification and route to `执行 测试 <N>`.

## Step 2 - Fixed Verification Order

Run in order:
1. build
2. type-check
3. lint
4. tests + coverage
5. security/log audit
6. node `verify.cmd`

Profile support:
- `quick`: 1/2/6
- `full`: all
- `pre-pr`: all

Any phase fail -> enter auto-fix loop.

## Step 3 - Auto-Fix Loop

Use `../references/checkpoint-rules.md`.

Rules:
- max 3 attempts
- hypothesis before each change
- one targeted change per attempt

Attempt 3 still fail -> stop and request user review.

## Step 4 - Scope Audit

Compare changed files with node target.
Unexpected files must be declared and explained.

## Step 5 - Persist State

Update `.ai/workflow/session.yaml` (legacy fallback: `.ai/workflow/session.md`):
- `last_completed_node: <N>`
- `current_node: <remove>`
- `next_action: <next node or verify>`
- `updated_at: <ISO-8601>`

Append `.ai/workflow/ledger.md` node summary:
- phase results
- verify evidence
- scope audit

If all nodes done:
- set `current_phase: verify`
- set `next_action: 验证`

## Step 6 - Checkpoint Output

```markdown
## Checkpoint: Node <N> Verified

Build: <pass/fail/skip>
Types: <pass/fail/skip>
Lint: <pass/fail/skip>
Tests: <pass/fail/skip>
Security: <pass/fail/skip>
Verify: <cmd + exit + signal>
Scope: <clean | declared extras>
Progress: <x/y>
Next: <exact next command>
```
