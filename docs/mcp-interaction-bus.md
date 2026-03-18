# MCP Interaction Bus 设计

状态：draft  
阶段：P2  
适用范围：`seeyue-mcp`、runtime、host wrapper、远程客户端

## 1. 目标

本文定义 MCP 在 interaction 体系中的定位：它不再只是 tools 集合，而是 `seeyue-workflows` 的交互总线，用于暴露状态、支持订阅、组织 prompts，并在支持的客户端中直接收集输入。

## 2. 核心原则

- runtime 是状态真相源
- MCP 是交互总线，不是主决策者
- resources 暴露状态，prompts 暴露入口，tools 执行动作
- 支持的客户端优先使用 `elicitation`
- 不支持的客户端回退到 `sy-interact` 或 text fallback

## 3. 当前基础

当前 `seeyue-mcp` 已经具备：

- `workflow://session`
- `workflow://task-graph`
- `workflow://journal`
- `workflow://dashboard`
- `workflow://questions`
- `workflow://inputs`

这为 interaction bus 提供了很好的起点，但仍缺：

- active interaction 资源
- interaction archive 资源
- subscribe / listChanged 语义
- structuredContent 一致输出
- 与 `elicitation` 的正式耦合

## 4. 目标资源模型

建议新增以下 resources：

| URI | Purpose |
|---|---|
| `workflow://interactions/active` | 当前待处理 interaction 聚合视图 |
| `workflow://interactions/index` | interaction 索引 |
| `workflow://interactions/{id}` | 单个 interaction 请求/响应详情 |
| `workflow://approvals` | 审批状态聚合 |
| `workflow://checkpoints` | checkpoint 索引 |
| `workflow://handoff/latest` | 最近 handoff capsule |
| `workflow://capability-gap` | 引擎能力差异报告 |
| `workflow://trust` | trust store 聚合 |

## 5. 订阅与变化通知

### 5.1 为什么要加 `subscribe` / `listChanged`

如果没有订阅机制，agent 或 host 只能频繁轮询 dashboard 或多个资源文件，成本高且容易错过关键交互状态变化。

### 5.2 建议的变化事件

建议至少支持：

- `interaction_requested`
- `interaction_resolved`
- `approval_state_changed`
- `restore_state_changed`
- `checkpoint_created`
- `handoff_created`

### 5.3 典型收益

- host 可在 interaction 创建时立即拉起 presenter
- 远程 MCP client 可实时看到 blocker 变化
- dashboard 不再依赖高频轮询

## 6. Prompt 层定位

Prompt 层应作为轻量技能桥接层，而不是承担交互状态本身。

建议：

- `prompts/list` 展示技能入口和交互辅助 prompt
- `prompts/get` 只按需提供 prompt 内容
- prompt 可引用 active interaction 摘要，但不直接承载最终状态

例如可以新增：

- `prompt://workflow/resolve-approval`
- `prompt://workflow/restore-guidance`
- `prompt://workflow/conflict-resolution`

## 7. Tool 层定位

Tool 层应执行动作，而不是承担展示逻辑。

建议输出始终包含：

- `content`
- `structuredContent`

其中 `structuredContent` 至少包含：

- `verdict`
- `reason_code`
- `blocking`
- `interaction_ref`
- `recommended_next`
- `artifacts`

## 8. `elicitation` 优先级

当 MCP 客户端支持 `elicitation` 时，建议优先走该路径，因为它更接近远程/嵌入式交互。

优先级建议：

1. MCP `elicitation`
2. 本地 `sy-interact`
3. text resource / prompt fallback

### 8.1 约束

- 所有 `elicitation` 都必须绑定 originating client request
- 敏感输入必须使用安全模式，不走普通 comment
- `elicitation` 只是输入渠道，不改变 interaction 真相源

## 9. `sy-interact` 与 MCP 的边界

### 9.1 `sy-interact`

负责：

- 本地 TTY UI
- 键盘事件
- 颜色渲染
- response 回写

### 9.2 MCP

负责：

- 远程状态暴露
- 订阅与变化通知
- 支持的客户端内收集输入
- 跨进程、跨宿主传输结构化对象

### 9.3 两者关系

- 不是互斥关系
- 是 native interactive client 与 local presenter 的双路径
- 真相源始终在 runtime store

## 10. 与现有资源的整合建议

建议保留现有资源，同时逐步把 `questions` / `inputs` 迁移到统一 interaction 视图中。

短期：

- `workflow://dashboard` 保留为聚合入口
- 新增 `workflow://interactions/active`
- `workflow://questions` / `workflow://inputs` 继续存在

中期：

- dashboard 中的 pending questions / inputs 改为来源于 interaction index

## 11. 建议新增 MCP tools

P2 建议考虑新增：

- `sy_list_interactions`
- `sy_read_interaction`
- `sy_resolve_interaction`
- `sy_probe_interaction_capability`

说明：

- 这些 tools 负责读写状态或桥接输入，不负责本地渲染
- 本地渲染仍由 `sy-interact` 承担

## 12. Non-Goals

- 不把所有本地交互都搬进 MCP
- 不让 MCP server 主进程直接承担 raw-mode TUI
- 不用 prompts 替代 interaction store

## 13. Top Risks

1. **高风险：MCP tools 与 interaction store 双向写入无统一约束**  
   缓解：所有写入都经 runtime/authorized bridge。

2. **中风险：resource 过多导致 agent 不知道先读什么**  
   缓解：坚持 `workflow://dashboard` 与 `workflow://interactions/active` 双入口。

3. **中风险：`elicitation` 与 `sy-interact` 同时抢占输入职责**  
   缓解：明确定义优先级与 originating request 绑定规则。

## 14. 参考依据

- `seeyue-mcp/src/resources/workflow.rs`
- `seeyue-mcp/src/prompts/registry.rs`
- `docs/interaction-runtime-integration.md`
- `workflow/interaction.schema.yaml`
- MCP Resources / Prompts / Elicitation  
  https://modelcontextprotocol.io/specification/2024-11-05/server/resources  
  https://modelcontextprotocol.io/specification/2024-11-05/server/prompts  
  https://modelcontextprotocol.io/specification/2024-11-05/client/elicitation
