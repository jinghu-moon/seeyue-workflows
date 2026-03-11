# Operation: 工作流 启动 <task-description>

Bootstrap unified workflow and route to the correct first phase.

## Precondition

- Task description provided

## Steps

1. Load `sy-constraints` then `sy-workflow-constraints`.
2. Ensure `.ai/init-report.md` exists:
   - if absent -> run `sy-code-insight: 初始化`
3. Run task analysis:
   - `sy-code-insight: 分析 <task-description>`
4. Decide first phase:
   - if explicit command is `工作流 探索` -> route to `sy-workflow: 探索`
   - if explicit command is `工作流 对标` -> route to `sy-workflow: 对标`
   - if explicit command is `工作流 构思` -> route to `sy-ideation`
   - if explicit command is `工作流 设计` -> route to `sy-design`
   - if explicit command is `工作流 工作树` -> route to `sy-worktree`
   - if explicit command is `工作流 调试` -> route to `sy-debug`
   - if multiple entry signals match -> ask one clarification question, then route
   - if vague/idea-only -> route to `sy-workflow: 探索`
   - if reference-driven refactor -> route to `sy-workflow: 对标`
   - if creative/new behavior/ambiguous -> route to `sy-ideation`
   - if design approval is missing -> route to `sy-design`
   - otherwise -> route to `sy-writing-plans`
5. Persist `.ai/workflow/session.yaml`:
   - create `.ai/workflow/` if absent
   - write current phase, next action, and timestamp
6. Output current phase + next command

## Routing Heuristics

Route to **Discover** when any condition matches:
- user only has high-level idea with no clear goal/scope/success metric
- requirement text lacks concrete actor + scenario + acceptance signal

Route to **Benchmark** when any condition matches:
- user explicitly wants to reference another project/system for refactor
- there is a clear external/internal target to compare against

Route to **Ideation** when any condition matches:
- user asks for idea/design/architecture choice
- multiple technical approaches exist with non-trivial tradeoffs

Route to **Design** when any condition matches:
- upstream discovery/benchmark/ideation already exists but architecture is not approved
- user has concrete task but no approved component/tech-stack/non-goal definition

Otherwise route to **Plan** directly.

## Session Snapshot Template

```yaml
schema: 2
run_id: wf-20260308-001
task: <task-description>
current_phase: discover|benchmark|ideation|design|worktree|plan
mode: normal
last_node: N0
next_action: <探索|对标|构思|设计|工作树|计划>
updated_at: 2026-03-08T00:00:00Z
```

## Output

```markdown
## Workflow Started

Task: <task-description>
Phase: <discover|benchmark|ideation|design|worktree|plan>
Next: <exact next command>
State: `.ai/workflow/session.yaml`
```