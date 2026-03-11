# Operation: Execute

Per-node execution loop. Modifies code. Checkpoints after each node.

## Precondition

- Plan phase completed and user approved
- Micro-Nodes list available
- Constraint snapshot from Plan phase available
- Phase boundary contract available (`entry_condition`, `exit_gate`, `rollback_boundary`)

## Loop (for each Node)

```
IF mode == parallel:
  0. Role Priming → load author prompt + stack persona
  1. Build execution groups by `parallel_group` + dependency order
  2. Parallel Preflight for each group
  3. Dispatch nodes in same group concurrently
  4. Run post-group merge verification
  5. Checkpoint by group, await "CONTINUE"
ELSE:
  FOR node in approved_nodes (ordered by depends_on):
    0. Role Priming → load author prompt + stack persona
    0.5 Node Readiness Gate → enforce TDD red gate when required
    0.6 Phase Boundary Gate → confirm entry_condition met and rollback path exists if required
    1. Research  → verify uncertain APIs via official docs
    2. Context   → sy-code-insight: 理解 <node.target>
    3. Execute   → apply code change
    4. Self-Audit→ check node output against phase scope + hard constraints
    5. Verify    → run node.verify (compile / test / lint / build)
    6. IF fail   → Auto-Fix (max 3 retries)
    7. IF 4th fail → STOP, report to user
    8. IF mode == normal  → Checkpoint: output progress, STOP, await "CONTINUE"
       IF mode == auto    → output progress, continue to next node
       IF mode == batch   → checkpoint every 3 nodes, then await "CONTINUE"
```

## Step Details

### 0. Role Priming (MUST)

- Load `.agents/skills/sy-code-insight/references/prompts/author.prompt.md`
- Resolve stack persona from:
  - `.ai/analysis/ai.report.json` → `project.language_stack` (preferred)
  - fallback: `.ai/init-report.md` + manifest/dependency signals
- Persona prompt text MUST be English and agent-oriented
- Keep user-facing progress output language unchanged by conversation rules

### 0.5 Node Readiness Gate (TDD)

- If node is behavior change/bug fix and `node.tdd_required` missing:
  - MUST STOP and request plan correction
- If `node.tdd_required == true`:
  - MUST run `node.red_cmd` first and confirm expected failure
  - implementation can start only after RED verified
- After implementation:
  - MUST run `node.green_cmd` and confirm pass
- If RED does not fail as expected → STOP and revise test before coding
- If `tdd_exception` is declared:
  - MUST check user approval marker before coding
  - MUST run fallback verification specified by exception

### 0.6 Phase Boundary Gate

- Before modifying node.target, MUST confirm phase `entry_condition` is satisfied.
- If node touches schema/data/public API:
  - MUST verify `rollback_boundary` is declared and usable.
  - MUST NOT execute node if restore path is missing.

### 1. Research (Zero-Guessing)

- MUST NOT invent API names, flags, or behavior
- IF uncertain about API details OR compile/type error indicates mismatch:
  - Search official docs first (e.g., docs.rs, framework official docs)
  - Record the source used in checkpoint notes
- IF fully certain and no uncertainty/error exists → skip active lookup

### Parallel Preflight (mode == parallel)

- For each `parallel_group`, MUST verify:
  - no file overlap across nodes
  - no dependency edge among group members
  - no shared mutable state risks
- Any preflight failure:
  - fallback to sequential execution for affected nodes
  - report fallback reason in checkpoint
- After each group completes:
  - MUST run `group_verify` post-group merge verification before next group
  - IF `group_verify` missing in plan, MUST STOP and request plan correction

### 2. Context Acquisition

- Call `sy-code-insight: 理解 <node.target>`
- MUST prefer incremental context (changed files + `.ai/insights` cache) before full re-read
- MUST validate README.AI.md `Source Manifest` (if present) before trusting doc evidence
- IF target file > 500 lines → signatures only
- Understand current interface before modifying

### 3. Code Change Rules

- Schema First: IF node involves API/interface change → define type/interface before implementation
- Atomic: each node produces a compilable, testable state
- MUST stay within current phase scope
- MUST NOT modify files outside node.target unless declared in plan

### 4. Self-Audit (Before Verify)

- Apply `sy-workflow-constraints` checklist first
- Confirm no out-of-scope/future-phase behavior is introduced
- Confirm node constraints from plan are satisfied
- Confirm implementation matches source-of-truth requirements

### 5. Verification (Hard Gate)

| Method | When | Pass Condition |
|--------|------|----------------|
| compile | Every node | Zero errors |
| test | Node has test target | All tests pass |
| lint | Style-sensitive changes | Zero warnings |
| build | Packaging/frontend nodes | Build succeeds |
| coverage | Behavior-change nodes | `actual >= required` |
| manual | UI / visual changes | User confirms |

No node is "complete" unless required verification passes.
No "success" claim is allowed without fresh command evidence.

### 6. Auto-Fix

See [references/checkpoint-rules.md](../references/checkpoint-rules.md) for full strategy.

- Retry 1: Root-cause evidence capture (error, stack, failing input)
- Retry 2: Broaden context (read related files)
- Retry 3: Root-cause debug pass (trace cause, then simplify approach if needed)
- Retry 4: STOP — report failure to user

### 7. Checkpoint Output

```
✅ Node N1 complete
  Target: src/types.rs
  Action: Defined DryRunConfig interface
  Research: docs.rs/winreg (set_raw_value)
  Evidence: cargo test -p core::redirect (exit 0)
  Update Mode: INCREMENTAL
  Delta Basis: changed=2, deleted=0, renamed=0, manifest-mismatch=no
  Report: .ai/analysis/ai.report.json updated (machine), .ai/analysis/ai.report.md updated (human)
  Self-Audit: scope=PASS, constraints=PASS
  Verify: compile ✓
  Coverage: 86% / 80%
  Progress: 1/3 nodes (33%)

Awaiting "CONTINUE" to proceed to N2.
```

### 8. Session Resume

IF session interrupted (context lost, timeout, crash):
1. User sends `继续` / `CONTINUE` in new session
2. Agent reads plan (from conversation or .ai/ context)
3. Identify last completed node (by checking file state + verify)
4. Resume from next incomplete node
5. Output: "Resumed from N{X}. {completed}/{total} nodes done."
