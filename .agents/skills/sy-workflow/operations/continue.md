# Operation: 工作流 继续

Resume unified workflow from persisted session state.

## Precondition

- `.ai/workflow/session.yaml` exists, or legacy `.ai/workflow/session.md` exists
- If missing -> instruct user to run `工作流 启动 <task-description>`

## Steps

1. Read workflow session state.
2. Read latest report metadata if present:
   - `.ai/analysis/ai.report.json` (`run.phase_id`, `run.node_id`, `generated_at`)
3. Resolve resume target by `current_phase`:
   - `discover` -> continue idea clarification until task shape is actionable
   - `benchmark` -> continue fit-gap and migration strategy refinement
   - `ideation` -> continue `sy-ideation`, then route to `设计`
   - `design` -> continue `sy-design` until explicit approval
   - `worktree` -> continue `sy-worktree` until workspace baseline is declared
   - `plan` -> continue `sy-writing-plans` (approval gate first)
   - `execute` -> continue `sy-executing-plans`
   - `verify` -> continue `sy-verification-before-completion`
   - `review` -> continue `sy-requesting-code-review`
   - `review_feedback` -> continue `sy-receiving-code-review`
   - `debug` -> continue `sy-debug`
   - `done` -> report completed state; suggest `工作流 启动 <task-description>` for new task
4. Update session state (`updated_at`, `next_action`, `last_node` if needed)
5. Output exact resumed command

## Resume Rules

- MUST re-check constraints before executing next step
- MUST preserve phase gate
- MUST report mismatch if session state and `ai.report.json` disagree

## Output

```markdown
## Workflow Resumed

Phase: <current_phase>
Last Node: <N?>
Next: <exact next command>
State Updated: `.ai/workflow/session.yaml`
```