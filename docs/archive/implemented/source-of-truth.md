# 事实源说明

## 目标

`seeyue-workflows` 使用“机器事实源优先，聊天记忆只做辅助手段”的原则。

1. 所有可执行规则必须先写入 machine-readable 文件。
2. 所有运行态推进必须先写入 `.ai/workflow/` 状态，再决定下一步。

## 机器事实源

以下文件是机器事实源：

- [runtime schema](../../../workflow/runtime.schema.yaml)
- [router spec](../../../workflow/router.spec.yaml)
- [policy spec](../../../workflow/policy.spec.yaml)
- [capabilities](../../../workflow/capabilities.yaml)
- [persona bindings](../../../workflow/persona-bindings.yaml)
- [file classes](../../../workflow/file-classes.yaml)
- [approval matrix](../../../workflow/approval-matrix.yaml)
- [hooks spec](../../../workflow/hooks.spec.yaml)
- [skills spec](../../../workflow/skills.spec.yaml)
- [hook contract schema](../../../workflow/hook-contract.schema.yaml)
- [validate manifest](../../../workflow/validate-manifest.yaml)
- [output templates spec](../../../workflow/output-templates.spec.yaml)

这些文件定义：

- phase / node / capability / persona 的关系
- TDD gate、review gate、approval gate
- `recommended_next` 的计算依据
- 并行 / 串行、超时、重试、恢复约束

修改这些文件后，必须重新验证相关 runtime、hook、adapter 行为。

## 运行态事实源

以下内容属于单次 run 的状态源：

- `.ai/workflow/session.yaml`
- `.ai/workflow/task-graph.yaml`
- `.ai/workflow/sprint-status.yaml`
- `.ai/workflow/journal.jsonl`
- `.ai/workflow/checkpoints/`
- `.ai/workflow/capsules/`

判断当前 run 进度时，优先看运行态事实源；判断规则定义时，优先看 `workflow/*.yaml`。

## 分发入口

对外分发时，需要同步以下入口：

- [Claude 入口](../../../CLAUDE.md)
- [Codex 入口](../../../AGENTS.md)
- [Gemini 入口](../../../GEMINI.md)
- [Claude settings](../../../.claude/settings.json)
- [Codex config](../../../.codex/config.toml)
- [Gemini settings](../../../.gemini/settings.json)
- [skill metadata](../../../.codex/skill-metadata.json)

对应 adapter 编译入口：

- [claude adapter](../../../scripts/adapters/claude-code.cjs)
- [codex adapter](../../../scripts/adapters/codex.cjs)
- [gemini adapter](../../../scripts/adapters/gemini-cli.cjs)

## 版本与同步

发布与同步边界以 [`../sync-manifest.json`](../../../sync-manifest.json) 为准。
版本关系说明见 [版本化策略](./versioning-policy.md)。

同步时至少确认：

- 需要分发的 docs 已进入 manifest
- 需要分发的 skills / hooks / tests 已进入 manifest
- `schema_version` 与 `workflow version` 没有混用
- 兼容声明与 `minimum_sync_version` 保持一致

## 阅读顺序

建议按以下顺序理解系统：

1. [V4 架构方案](../../architecture-v4.md)
2. [runtime schema](../../../workflow/runtime.schema.yaml)
3. [router spec](../../../workflow/router.spec.yaml)
4. [policy spec](../../../workflow/policy.spec.yaml)
5. [版本化策略](./versioning-policy.md)
6. [运行手册](./operations-runbook.md)
