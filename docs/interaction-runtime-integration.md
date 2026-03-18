# Interaction 与 Runtime 集成设计

状态：draft  
阶段：P1  
适用范围：`scripts/runtime/*`、`.ai/workflow/*`、host wrapper、`sy-interact`  

## 1. 目标

本文定义 `interaction_request` / `interaction_response` 如何接入现有 runtime，使 `sy-interact` 成为控制平面的一部分，而不是独立存在的 UI 工具。

目标是回答五个问题：

- runtime 在什么条件下生成 interaction
- interaction 如何持久化
- interaction 如何阻塞或恢复流程
- interaction 如何与 `recommended_next`、`approval_pending`、`restore_pending` 协同
- interaction 结束后 runtime 如何恢复推进

## 2. 当前基础

当前仓库已经具备：

- `.ai/workflow/session.yaml` 作为当前 run 的主状态
- `.ai/workflow/task-graph.yaml` 作为 phase/node 图
- `.ai/workflow/journal.jsonl` 作为 append-only 事件流
- `recommended_next`、`restore_reason`、`approval_pending` 等核心状态对象
- `workflow://dashboard`、`workflow://questions`、`workflow://inputs` 等资源能力

这意味着 interaction 层不需要从零起步，而应嵌入现有 durable store 与 router 语义中。

## 3. 设计原则

- runtime 仍然是唯一决策中心
- interaction 是 durable state，不是临时 prompt
- blocker-first 优先级高于普通 `recommended_next`
- presenter 结果必须能被 journal 与 checkpoint 捕获
- interaction 必须支持恢复、超时、取消与冲突澄清

## 4. 建议的持久化布局

建议在 `.ai/workflow/` 下增加：

```text
.ai/workflow/
  interactions/
    active.json
    requests/
      ix-*.json
    responses/
      ix-*.json
    archive/
      ix-*.json
```

### 4.1 文件职责

- `active.json`
  - 当前待处理 interaction 的索引
  - 只保留极小聚合信息，便于 host 快速发现 pending interaction
- `requests/ix-*.json`
  - 原始请求对象
- `responses/ix-*.json`
  - presenter 写出的响应对象
- `archive/ix-*.json`
  - 已关闭交互的归档快照，可选

### 4.2 为什么不用单个大文件

不建议把所有 interaction 全塞进一个 JSON 数组文件中，原因：

- 并发下更难安全更新
- 不利于调试与回放
- 不利于逐条归档
- 与现有 `journal.jsonl` 的 append-only 模型不一致

## 5. Session 扩展建议

建议在 `session.yaml` 中增加一个最小 interaction 状态块：

```yaml
interaction:
  active_id: ix-20260318-001
  pending_count: 1
  last_resolved_id: ix-20260318-000
  blocking_kind: approval
  blocking_reason: destructive_write_requires_approval
```

### 5.1 字段语义

- `active_id`
  - 当前唯一焦点 interaction
- `pending_count`
  - 当前等待处理的 interaction 数量
- `last_resolved_id`
  - 最近一次已完成交互
- `blocking_kind`
  - 与 interaction 相关的 blocker 类型
- `blocking_reason`
  - 面向 runtime 的 reason code

## 6. 生命周期模型

建议 interaction 生命周期至少包含：

- `pending`
- `presented`
- `answered`
- `cancelled`
- `expired`
- `failed`
- `superseded`

### 6.1 状态流

```text
runtime creates request
  -> pending
host launches sy-interact
  -> presented
user submits answer
  -> answered
runtime consumes response
  -> archived
```

异常路径：

```text
pending -> expired
pending -> failed
presented -> cancelled
pending -> superseded
```

## 7. 触发条件

runtime 应仅在以下条件下生成 interaction：

### 7.1 审批类

当 policy / hook / runtime 判断某动作需要明确人类授权时，生成：

- `approval_request`

### 7.2 恢复类

当 `restore_pending = true`，且必须要求用户选择恢复方式时，生成：

- `restore_request`

### 7.3 信息缺失类

当 runtime 无法继续推进，且缺少关键参数或范围定义时，生成：

- `question_request`
- `input_request`

### 7.4 冲突类

当用户 comment 与结构化答案冲突，或状态存在多条候选恢复路径时，生成：

- `conflict_resolution`

## 8. blocker-first 优先级

interaction 与 runtime blocker 的建议优先级：

1. `restore_pending`
2. active `approval_request`
3. active `conflict_resolution`
4. active `question_request`
5. active `input_request`
6. 普通 `recommended_next`

含义是：只要存在更高优先级 blocker，就不应继续执行新的写入、阶段推进或自动 loop。

## 9. 与 `recommended_next` 的关系

### 9.1 生成规则

`recommended_next` 仍由 router/runtime 生成，但 interaction 层必须把它包装成对用户可见的动作上下文。

### 9.2 表示规则

建议当 interaction 存在时：

- `recommended_next[0]` 指向“处理当前 interaction”
- 后续项 MAY 保留恢复后动作建议

例如：

```yaml
recommended_next:
  - type: resolve_interaction
    params:
      interaction_id: ix-20260318-001
    reason: approval required before continuing
  - type: resume_node
    params:
      node_id: P1-N2
    reason: continue after approval
```

### 9.3 展示规则

用户界面先看 blocker，再看 `recommended_next`，而不是反过来。

## 10. 与 journal / checkpoint 的关系

### 10.1 建议新增 journal 事件

建议 interaction 相关事件标准化为：

- `interaction_requested`
- `interaction_presented`
- `interaction_answered`
- `interaction_cancelled`
- `interaction_expired`
- `interaction_failed`
- `interaction_conflict_detected`

### 10.2 checkpoint 约束

当存在 active interaction 时：

- checkpoint MUST 包含 `active_id`
- handoff capsule SHOULD 包含 pending interaction 摘要
- restore 后 SHOULD 先恢复 interaction 状态，再恢复普通执行状态

## 11. Host Wrapper 集成流程

推荐流程：

1. runtime 写入 `requests/ix-*.json`
2. runtime 更新 `active.json` 与 `session.interaction`
3. host 监听到 active interaction
4. host 选择 `elicitation` 或 `sy-interact`
5. 若走 `sy-interact`，写入 `responses/ix-*.json`
6. runtime 消费响应并推进状态
7. runtime 归档 interaction 并刷新 `recommended_next`

## 12. 非 TTY 与远程交互

### 12.1 本地 TTY

优先使用 `sy-interact`。

### 12.2 支持 MCP 交互的客户端

优先使用：

- `elicitation`
- `subscribe` / `listChanged`
- 结构化 tool / resource response

### 12.3 无 TTY 且无 MCP 交互支持

降级为：

- text prompt
- JSONL / resource fallback
- host 记录 `interaction_pending` 并等待人工后续处理

## 13. Data Model

interaction store 与 runtime store 的最小映射如下：

| Runtime State | Interaction Meaning |
|---|---|
| `session.approvals.pending` | 可以投影为 active `approval_request` |
| `session.recovery.restore_pending` | 可以投影为 active `restore_request` |
| `questions.jsonl` | 可逐步收敛到 `question_request` |
| `input_requests.jsonl` | 可逐步收敛到 `input_request` |
| `sprint_status.recommended_next` | 交互结束后的推荐动作来源 |

## 14. Integration Points

### 14.1 Runtime ↔ Hook Client

Hook Client 只上报 blocker 与 context，不直接维护 interaction store。

### 14.2 Runtime ↔ Host Wrapper

Host 只发现、拉起 presenter、回收 response，不决定业务语义。

### 14.3 Runtime ↔ `sy-interact`

通过 request/response 文件交换数据，不通过 prompt 自由文本协商。

### 14.4 Runtime ↔ MCP

MCP 提供 interaction 资源与远程输入路径，但 runtime 仍是主状态源。

## 15. Non-Goals

- 不在 P1 阶段彻底移除 `questions.jsonl` 与 `input_requests.jsonl`
- 不要求所有 interaction 都必须走 TUI
- 不在 P1 阶段实现复杂并发多 interaction UI

## 16. Top Risks

1. **高风险：interaction 与既有 approvals/questions/input_requests 双轨并存**  
   缓解：先做投影层，再渐进迁移。

2. **中风险：host wrapper 不同实现导致状态消费不一致**  
   缓解：统一 `active.json` 和 response file 协议。

3. **中风险：用户取消后 runtime 行为不一致**  
   缓解：明确 `cancelled` 进入 router 的处理规则。

## 17. 参考依据

- `workflow/runtime.schema.yaml`
- `workflow/router.spec.yaml`
- `seeyue-mcp/src/resources/workflow.rs`
- `docs/interaction-tui-architecture.md`
- `workflow/interaction.schema.yaml`
- MCP lifecycle / resources  
  https://modelcontextprotocol.io/specification/2024-11-05/basic/lifecycle  
  https://modelcontextprotocol.io/specification/2024-11-05/server/resources
