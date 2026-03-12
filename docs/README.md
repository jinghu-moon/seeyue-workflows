# 文档索引

## 核心文档

- [V4 架构方案](./architecture-v4.md)：说明执行引擎、约束分层、TDD 合约与运行态模型。
- [接入指南](./adoption-guide.md)：说明如何把 `seeyue-workflows` 接入目标仓库。
- [运行手册](./operations-runbook.md)：说明巡检、恢复、审计、发布的操作顺序。
- [Hooks 开发流程说明](./hooks-guide.md)：按开发流程说明 hooks 规范、实现、适配器与验证顺序。
- [发布检查清单](./release-checklist.md)：说明版本发布前后必须检查的命令与产物。
- [版本化策略](./versioning-policy.md)：说明 `workflow version`、`adapter version`、`schema_version` 的关系。
- [事实源说明](./source-of-truth.md)：说明 machine source of truth、运行态状态源与同步边界。

## 实施材料

- [V5 实施计划](./implementation-plan-v5.md)
- [格式策略](./format-strategy.md)
- [仓库结构](./repo-structure.md)
- [迁移映射](./migration-map.md)
- [机器对齐清单](./machine-alignment-checklist.md)
- [Hooks 改造建议清单](./hooks-improvement-checklist.md)

## 评审与修复

- [V4 严格自我评审（2026-03-09）](./v4-self-review-20260309.md)
- [V4 修复 Backlog（2026-03-09）](./v4-remediation-backlog-20260309.md)

## 规范草案

- [router 中文草案](./router-spec-draft.md)
- [router 后续清单](./router-follow-up-checklist.md)
- [测试门规范草案 v2](./test-gate-spec-v2.md)
- [session schema 说明](./session-schema.md)

## 机器事实源

- [runtime schema](../workflow/runtime.schema.yaml)
- [router spec](../workflow/router.spec.yaml)
- [policy spec](../workflow/policy.spec.yaml)
- [capabilities](../workflow/capabilities.yaml)
- [persona bindings](../workflow/persona-bindings.yaml)
- [file classes](../workflow/file-classes.yaml)
- [approval matrix](../workflow/approval-matrix.yaml)
