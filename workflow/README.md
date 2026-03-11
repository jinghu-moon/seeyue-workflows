# Workflow Specs

## 说明

- `workflow/` 是 `seeyue-workflows` 的机器真源目录。
- 这里的 `*.yaml` 定义 workflow 的逻辑规则、路由契约、运行时状态结构与策略边界。
- `docs/*.md` 负责给人审核、解释与讨论；`workflow/*.yaml` 负责给 runtime、adapter、validator 消费。

## 文件职责

- `runtime.schema.yaml`：定义 `.ai/workflow/` 运行时状态资产的结构边界。
- `router.spec.yaml`：定义 phase、persona、adapter 与 `recommended-next` 路由语义。
- `hooks.spec.yaml`：定义 hook 矩阵与命令分类边界。
- `policy.spec.yaml`：定义审批矩阵、TDD 门禁、风险分类与 hook 判定语义。

## 边界

- 不在这里写长篇解释性 prose。
- 不把 vendor 文件当成真源。
- 不在未批准的情况下扩展额外 schema。

## Source of Truth

- human review: `docs/architecture-v4.md`
- machine source of truth: `workflow/*.yaml`
- deployment artifacts: `CLAUDE.md` / `AGENTS.md` / `GEMINI.md` / engine glue
