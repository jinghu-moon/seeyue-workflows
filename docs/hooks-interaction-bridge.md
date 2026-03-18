# Hooks 与 Interaction Bridge 设计

状态：draft  
阶段：P2  
适用范围：`scripts/hooks/*`、`scripts/runtime/hook-client.cjs`、runtime kernel、host wrapper

## 1. 目标

本文定义 Hooks 如何把阻塞、审批、恢复、冲突等信号桥接到 interaction 层，同时保持 Hooks 足够薄、足够快、足够稳定。

## 2. 核心原则

- Hooks 只做物理边界拦截
- Hook Client 负责 IPC、封装与错误归类
- Runtime Kernel 负责 interaction 生成
- Host 负责拉起 `sy-interact`
- Hooks 不直接控制本地 UI

## 3. 当前基础

仓库已具备：

- `workflow/hooks.spec.yaml` 定义事件矩阵
- `workflow/hook-contract.schema.yaml` 定义 hook IPC 合同
- `scripts/runtime/hook-client.cjs` 作为统一桥接入口
- `scripts/hooks/*` 一组 thin hooks
- `scripts/runtime/engine-kernel.cjs` 作为决策汇聚层

这意味着 interaction bridge 不需要重建一套新链路，而是应沿着现有 Hook → Hook Client → Kernel 的方向收口。

## 4. 目标桥接链路

```text
Native Hook Event
  -> Thin Hook Script
  -> hook-client.cjs
  -> Runtime Kernel
  -> interaction_request (optional)
  -> host wrapper
  -> sy-interact
  -> interaction_response
  -> Runtime Kernel resumes
```

## 5. Hook 职责边界

### 5.1 Thin Hook Script 应负责

- 接收原生事件
- 提取最小必要上下文
- 调用 Hook Client
- 把 verdict 翻译回引擎原生格式

### 5.2 Thin Hook Script 不应负责

- 直接读写 interaction store
- 直接拉起 `sy-interact`
- 计算 `recommended_next`
- 决定审批范围
- 决定恢复路径
- 生成长篇用户文案

## 6. failure_mode 建议

建议在 `workflow/hooks.spec.yaml` 中为各 Hook 显式增加：

- `hard_gate`
- `advisory`
- `telemetry`

建议映射：

| Event | Suggested failure_mode | Reason |
|---|---|---|
| `PreToolUse:Write|Edit` | `hard_gate` | 不能 silently fail-open |
| `PreToolUse:Bash` | `hard_gate` | 高风险命令需要强边界 |
| `Stop` | `hard_gate` | 涉及恢复与交接 |
| `SessionStart` | `advisory` | 失败不应阻止进入会话 |
| `BeforeToolSelection` | `advisory` | 更像优化性前置检查 |
| `AfterModel` | `advisory` | 更多是诊断/检查 |
| `PostToolUse:*` | `telemetry` | 失败主要影响证据，不应破坏主流程 |

## 7. interaction 生成规则

### 7.1 Hook 直接结果类型

Hook Client 收到 native hook 结果后，运行时应将其归一为以下几类：

- `allow`
- `block`
- `block_with_approval_request`
- `force_continue`
- `ask_question`
- `request_input`

### 7.2 interaction 生成时机

仅当 verdict 需要显式用户动作时，才应生成 interaction：

- `block_with_approval_request` -> `approval_request`
- `ask_question` -> `question_request`
- `request_input` -> `input_request`
- `block` with restore context -> `restore_request`
- `block` with semantic conflict -> `conflict_resolution`

### 7.3 interaction 不生成时机

以下情况不应创建 interaction：

- 单纯 `allow`
- 单纯 post-tool telemetry 失败
- 可被 runtime 自动恢复的低风险异常
- 仅需写 journal 的证据场景

## 8. Hook Client 扩展建议

`hook-client.cjs` 应新增或强化以下职责：

- 标准化 `reason_code`
- 标准化 `risk_level`
- 把 native hook event 映射到统一 `origin`
- 区分“策略拒绝”和“执行故障”
- 把“需要人工动作”的结果明确交给 runtime interaction builder

建议新增统一 decision envelope 字段：

- `interaction_required`
- `interaction_kind`
- `blocking_kind`
- `reason_code`
- `risk_level`
- `scope`
- `recommended_next`

## 9. 与 host 的边界

Hook 结束后不应直接阻塞在本地 UI 上等待用户输入。正确流程是：

1. Hook Client 返回 block / request 结果
2. runtime 写 interaction request
3. host 发现 pending interaction
4. host 拉起 `sy-interact`
5. runtime 消费 response 后再恢复主流程

这样可以避免：

- hook 生命周期被 UI 占住
- 不同引擎的 hook timeout / behavior 失控
- 原生 hook 执行模型与本地 TUI 互相耦合

## 10. 与 journal / evidence 的关系

Hook bridge 除了交互桥接，还必须保留证据链。

建议 interaction 相关 journal 事件包含：

- originating hook event
- tool name / file scope
- hook verdict
- generated interaction id
- host presentation outcome

## 11. 需要新增的 contract tests

建议补以下测试：

- `hard_gate` hook failure 不再 fail-open
- `approval_request` 可从 hook verdict 稳定生成
- `restore_request` 从 `Stop` blocker 稳定生成
- post-tool telemetry failure 不生成 interaction
- host 未处理 interaction 时 runtime 保持阻塞
- interaction response 被消费后 hook 链路可恢复执行

## 12. Non-Goals

- 不在 Hooks 层实现完整 interaction 状态机
- 不在 Hooks 层实现复杂 UI 行为
- 不在 Hooks 层持久化完整 response 对象

## 13. Top Risks

1. **高风险：Hook 直接拉起 UI 导致生命周期混乱**  
   缓解：强制 host 驱动 presenter。

2. **高风险：`hard_gate` 仍存在隐式 fail-open**  
   缓解：引入 `failure_mode` 并补 contract tests。

3. **中风险：Hook Client decision envelope 不够统一**  
   缓解：集中在 Hook Client 做结构化归一。

## 14. 参考依据

- `workflow/hooks.spec.yaml`
- `workflow/hook-contract.schema.yaml`
- `scripts/runtime/hook-client.cjs`
- `scripts/runtime/engine-kernel.cjs`
- `refer/skills-and-hooks-architecture-advisory.md`
- Claude Code Hooks  
  https://docs.anthropic.com/en/docs/claude-code/hooks
