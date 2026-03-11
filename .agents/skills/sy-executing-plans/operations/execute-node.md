# Operation: Execute Node
# Trigger: `执行 节点 <N>`

Implementation-only operation for a single node.

## Step 0 - Role Prompt

Load:
- `../references/personas/author.md`

Resolve stack from:
1. `.ai/workflow/session.yaml` (legacy fallback: `.ai/workflow/session.md`)
2. `.ai/analysis/ai.report.json`
3. signal files (`Cargo.toml`, `package.json`, `tsconfig.json`)

## Step 1 - Resolve Node

1. Resolve `NODE_ID` from command argument.
2. If absent, read the current node from `session.yaml` (legacy fallback: `session.md`), else pick the first incomplete node from plan.
3. Read plan node fields:
   - `target`
   - `action`
   - `verify.cmd`
   - `tdd_required`
   - `depends_on`

If required fields missing: stop and request plan fix.

## Step 2 - Preflight Gates

1. Dependency gate: all `depends_on` nodes must be completed.
2. TDD preflight:
   - if `tdd_required=true`, require `current_node.red_verified=true`.
3. Research-first gate:
   - for new dependency/net-new utility, ensure decision record exists (`adopt|extend|build`).
4. Freshness gate:
   - if target understanding is stale, run `sy-code-insight` read before edit.

## Step 3 - Implement Node

Rules:
- modify only node target files
- no future-node scope
- no guessed API names; verify against docs first
- keep implementation compilable/type-correct

## Step 4 - Self Reflection (MUST)

Before checkpoint, verify:
1. node action fully satisfied
2. no obvious edge-case miss
3. no out-of-scope side effects
4. no unverified assumptions

If issue found, fix first then continue.

## Step 5 - Write Checkpoint

Update `.ai/workflow/session.yaml` (legacy fallback: `.ai/workflow/session.md`):
- `current_phase: execute`
- `current_node.id: <NODE_ID>`
- `current_node.target: <target>`
- `current_node.tdd_required: <true|false>`
- `current_node.red_verified: false`
- `current_node.verify_cmd: <verify command>`
- `updated_at: <ISO-8601>`

Output:

```markdown
## Checkpoint: Node <N> Implementation

Target: <file(s)>
Action: <what changed>
Scope Drift: <none | list>
TDD: <required/not required>
Next: 执行 测试 <N>
```
