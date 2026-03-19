# 文档索引

## 核心文档

- [V4 架构方案](./architecture-v4.md)：说明执行引擎、约束分层、TDD 合约与运行态模型。
- [接入指南](./archive/implemented/adoption-guide.md)：说明如何把 `seeyue-workflows` 接入目标仓库。
- [运行手册](./archive/implemented/operations-runbook.md)：说明巡检、恢复、审计、发布的操作顺序。
- [Hooks 开发流程说明](./archive/implemented/hooks-guide.md)：按开发流程说明 hooks 规范、实现、适配器与验证顺序。
- [发布检查清单](./archive/implemented/release-checklist.md)：说明版本发布前后必须检查的命令与产物。
- [版本化策略](./archive/implemented/versioning-policy.md)：说明 `workflow version`、`adapter version`、`schema_version` 的关系。
- [事实源说明](./archive/implemented/source-of-truth.md)：说明 machine source of truth、运行态状态源与同步边界。

## 实施材料

- [V5 实施计划](./implementation-plan-v5.md)
- [格式策略](./archive/implemented/format-strategy.md)
- [仓库结构](./archive/implemented/repo-structure.md)
- [迁移映射](./archive/implemented/migration-map.md)
- [机器对齐清单](./machine-alignment-checklist.md)
- [Hooks 改造建议清单](./hooks-improvement-checklist.md)

## 评审与修复

- [V4 严格自我评审（2026-03-09）](./v4-self-review-20260309.md)
- [V4 修复 Backlog（2026-03-09）](./v4-remediation-backlog-20260309.md)

## 规范草案

- [router 中文草案](./archive/outdated/router-spec-draft.md)
- [router 后续清单](./router-follow-up-checklist.md)
- [测试门规范草案 v2](./archive/outdated/test-gate-spec-v2.md)
- [session schema 说明](./archive/outdated/session-schema.md)

## 机器事实源

- [runtime schema](../workflow/runtime.schema.yaml)
- [router spec](../workflow/router.spec.yaml)
- [policy spec](../workflow/policy.spec.yaml)
- [capabilities](../workflow/capabilities.yaml)
- [persona bindings](../workflow/persona-bindings.yaml)
- [file classes](../workflow/file-classes.yaml)
- [approval matrix](../workflow/approval-matrix.yaml)

---

## Symbol-First & MCP Dispatch 专题（2026-03-19）

> 基于 Serena 深度分析形成的下一阶段架构演进基线。四份文档构成完整工程闭环：洞察 → 分析 → 设计 → 执行。

| 层级 | 文档 | 定位 | 状态 |
|------|------|------|------|
| 北极星 | [symbol-first-north-star.md](./symbol-first-north-star.md) | 架构借鉴总纲：为什么学 Serena、学什么、不学什么 | ✅ 已通过审核 |
| Gap 分析 | [symbol-first-gap-analysis.md](./symbol-first-gap-analysis.md) | seeyue-mcp symbol-first 能力差距与补齐路线 | ✅ 已通过审核 |
| 设计契约 | [symbol-first-dispatch-design.md](./symbol-first-dispatch-design.md) | dispatch 层、ToolMetadata、compat、active filter 机器契约 | ✅ 已通过审核 |
| 执行清单 | [symbol-first-task-list.md](./symbol-first-task-list.md) | TDD 节点式任务清单（16 节点，含 DAG、gate、red/green cmd） | ✅ 已通过审核 |
| 执行记录 | [symbol-first-execution-record.md](./symbol-first-execution-record.md) | 逐节点验收证据记录模板 | 待填写 |

### 快速入口

- **理解背景**：先读 [symbol-first-north-star.md §一句话总结](./symbol-first-north-star.md#一句话总结)
- **了解差距**：读 [symbol-first-gap-analysis.md §四、Gap 优先级矩阵](./symbol-first-gap-analysis.md)
- **开始实施**：以 [symbol-first-task-list.md](./symbol-first-task-list.md) 为唯一执行清单
- **记录进度**：填写 [symbol-first-execution-record.md](./symbol-first-execution-record.md)

### 实施节点概览

```
Phase A（P0，~2天）：导航基础
  A-N1  LSP documentSymbol 底层接口
  A-N2  tree-sitter 符号树提取 + name_path 生成
  A-N3  sy_get_symbols_overview 工具
  A-N4  sy_find_symbol 工具（基础实现）
  A-N4b sy_find_symbol 接入 index.json 加速层
  A-N5  discover_server() 补全 10 种语言
  A-N6  .seeyue/index.json 项目符号索引
  A-N7  SessionStart hook 触发增量索引更新

Phase B（P1，~2天）：编辑精化
  B-N1  sy_find_referencing_symbols
  B-N2  sy_replace_symbol_body
  B-N3  sy_insert_after/before_symbol

Phase M（~2天，可与 A 并行）：MCP Dispatch 重构
  M-N1  ToolMetadata 注册表
  M-N2  tools/list 自动生成
  M-N3  server/dispatch.rs 统一入口
  M-N4  server/compat.rs 客户端兼容层
  M-N5  active_tools HashSet + Active Filter
```
