# Operation: Plan

Task decomposition into Micro-Nodes. Read-only phase — no code changes.

## Precondition

- User provides task description (e.g., "Add dry-run mode to redirect engine")
- Optional but recommended: user provides phase ID and spec/plan path (e.g., `P1`, `EnvMgr_实施方案.md`)
- `sy-workflow-constraints` loaded and available for this run
- If task is creative/new behavior/ambiguous, Ideation phase MUST be approved first

## Steps

0. Design Gate Check
   - IF task requires Ideation gate and no approved design exists:
     - STOP and request `构思 <topic>`
1. Role Priming
   - Load `.agents/skills/sy-code-insight/references/prompts/planner.prompt.md`
   - Resolve stack persona via `.ai/analysis/ai.report.json.project.language_stack` (fallback `.ai/init-report.md`)
   - Prompt language MUST be English and agent-oriented
2. Load source-of-truth requirements
   - IF spec/plan document is provided → read it fully
   - Extract current phase scope: In Scope / Out of Scope / Hard Constraints
   - IF no phase specified → ask user to confirm current phase before decomposition
3. Load constraint profile
   - Load `sy-constraints` baseline first, then `sy-workflow-constraints`
   - Build run-level snapshot: Source of Truth / Phase Gate / Zero-Guessing / Validation Gate
4. Check .ai/init-report.md exists
   - IF absent → execute `sy-code-insight: 初始化`
   - IF outdated (inference) → note staleness, proceed with code as truth
5. Execute `sy-code-insight: 分析 <task-description>`
   - Receive: impacted files, interfaces, risks, test strategy
6. Decompose task into Micro-Nodes within current phase only
7. For each node, define verification command and constraint check points
   - For behavior changes/bug fixes, MUST add TDD gates (RED command + GREEN command)
   - If TDD cannot apply, MUST declare `tdd_exception` + alternative verification and request explicit user approval
   - If node is independent and disjoint from others, MAY assign `parallel_group`
   - For current phase, MUST declare phase boundary contract:
     - `entry_condition`
     - `exit_gate` (`cmd`, `pass_signal`, `coverage_min`)
     - `rollback_boundary` (`revert_nodes`, `restore_point`)
8. Generate baseline machine report:
   - run `pwsh -File ".agents/skills/sy-code-insight/scripts/generate-ai-report.ps1" -Task "<task-description>" -UpdateMode "<mode>" -ChangedFiles <n> -DeletedFiles <n> -RenamedFiles <n> -PhaseId "<Px>" -NodeId "N0" -ReportName "<feature>" -Compile skip -Test skip -Lint skip -Build skip`
9. Output plan for user approval (include report paths)
10. STOP — await user confirmation before execute phase

## Micro-Node Definition

Each node is an atomic, independently verifiable code change.

| Field | Description |
|-------|-------------|
| id | Sequential: N1, N2, N3... |
| target | File path(s) to modify |
| action | What to do (create / modify / delete) |
| interface | Schema change if any (define before implement) |
| verify | How to verify (compile / test / lint / manual) |
| constraints | Which hard constraints this node must satisfy |
| depends_on | Previous node IDs this depends on |
| tdd_required | true/false for behavior changes and bug fixes |
| red_cmd | Command that MUST fail before implementation (if tdd_required) |
| green_cmd | Command that MUST pass after implementation (if tdd_required) |
| tdd_exception | Optional exception object: reason + approval + fallback verify |
| parallel_group | Optional group id (only for independent nodes) |
| group_verify | Post-group merge verification command (required when parallel_group is set) |
| rollback_boundary | Required for schema/data/public API touching nodes |

## Decomposition Rules

- MUST order: interface changes → implementation → tests → docs
- IF API change → first node MUST be schema/interface definition only
- IF cross-module → one node per module boundary
- MUST keep all nodes inside current phase scope
- MUST explicitly mark out-of-scope items as deferred, not silently implemented
- IF API uncertainty exists → add research action before implementation in that node
- IF behavior changes/bug fixes exist → MUST include TDD red/green node actions
- IF node touches schema/data/public API → MUST include rollback boundary declaration
- IF `tdd_exception` is used:
  - MUST include explicit reason
  - MUST include fallback verification (`test|integration|manual with checklist`)
  - MUST include user approval marker
- `parallel_group` can be set only when:
  - no file overlap
  - no shared mutable state
  - no dependency edge
- For `parallel_group`:
  - group members SHOULD have homogeneous verify type
  - each group MUST have `group_verify` post-group merge verification command
- Max 8 nodes per plan; larger tasks → split into sub-tasks

## Output Format

```
## Plan: <task-description>

Phase: <P1/P2/...>
Source of Truth: <spec path or "conversation requirements">
Constraint Profile: sy-workflow-constraints (loaded)
Phase Boundary:
  entry_condition:
    - ...
  exit_gate:
    cmd: ...
    pass_signal: ...
    coverage_min: <int>%|n/a
  rollback_boundary:
    revert_nodes: [N?]
    restore_point: ...
Scope Gate:
  - In Scope: ...
  - Out of Scope: ...

Context: <1-sentence from sy-code-insight 分析 output>
Impacted: <file count> files, Risk: <High/Med/Low>

### Nodes

| # | Target | Action | Verify | Constraints | Depends |
|---|--------|--------|--------|-------------|---------|
| N1 | src/types.rs | Define DryRunConfig interface | compile | schema-first | — |
| N2 | src/redirect.rs | Implement dry_run() | compile + test | no future-scope changes | N1 |
| N3 | tests/redirect_test.rs | Add dry-run test cases | test | no behavior drift | N2 |

Awaiting approval. Reply "执行" to proceed.

Report:
  - .ai/analysis/ai.report.md
  - .ai/analysis/ai.report.json
```
