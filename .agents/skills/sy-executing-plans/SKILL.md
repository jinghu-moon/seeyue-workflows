---
name: sy-executing-plans
description: Use when an approved plan exists and implementation should proceed node-by-node with split execution operations (node/test/verify) and checkpointed modes.
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
  - Glob
  - Grep
  - WebSearch
argument-hint: "[action, node]"
disable-model-invocation: false
---

# Executing Plans

Execute approved plan nodes with strict split operations:
- `execute-node`: implementation only
- `execute-test`: TDD only
- `execute-verify`: verification and checkpoint

## Trigger

Use when:
- user requests `执行` / `自动执行` / `批处理执行` / `并行执行`
- user requests `执行节点` / `运行测试` / `验证节点`
- plan has been approved and execution phase begins

## When NOT to use

- no approved plan exists
- design/plan approval gate is missing
- user asks for final completion verification (route to `sy-verification-before-completion`)
- user asks for review feedback processing (route to `sy-receiving-code-review`)

## Iron Rule

```text
DO NOT MERGE NODE/TEST/VERIFY INTO ONE STEP.
DO NOT CLAIM NODE COMPLETE WITHOUT execute-verify EVIDENCE.
```

## 3-Split Operation Contract

| Command | Operation |
|---|---|
| `执行` / `执行 节点 N3` | `operations/execute-node.md` |
| `执行 测试 N3` | `operations/execute-test.md` |
| `执行 验证 N3` | `operations/execute-verify.md` |

## Execution Modes

| Mode | Behavior |
|---|---|
| `normal` | 每个节点验证后暂停，等待继续 |
| `auto` | 连续执行，失败或预算命中即停 |
| `batch` | 每 3 个通过节点暂停 |
| `parallel` | 仅对安全并行组并行，失败回退串行 |

For `auto/batch/parallel`, write default loop budget to session:
- `loop_budget_max_nodes: 5`
- `loop_budget_max_minutes: 30`
- `loop_budget_max_consecutive_failures: 2`

## Mandatory Dependencies

- `sy-constraints`
- `sy-workflow-constraints`
- `sy-code-insight`
- `sy-writing-plans`

## References

- [operations/execute-node.md](operations/execute-node.md)
- [operations/execute-test.md](operations/execute-test.md)
- [operations/execute-verify.md](operations/execute-verify.md)
- [references/checkpoint-rules.md](references/checkpoint-rules.md)
- [references/personas/author.md](references/personas/author.md)

## Guardrails

- MUST keep execution within current node scope.
- MUST NOT commit unless user explicitly requests commit.
- MUST run `execute-test` before `execute-verify` for `tdd_required=true` nodes.
- MUST record session state after each node checkpoint.
- MUST route all-node-complete state to `sy-verification-before-completion`.
