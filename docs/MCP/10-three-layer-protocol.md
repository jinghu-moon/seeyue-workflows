# 三层联动协议（Three-Layer Collaboration Protocol）

> 来源：`docs/MCP/08-implementation-plan.md` §7 + `09-p3-implementation-plan.md` §2
> 编写日期：2026-03-17

---

## 1. 协议目标

本文档定义 MCP / Skills / Hooks 三层的**职责边界**、**交互协议**和**证据链格式**，消除 P0-P2 阶段遗留的职责模糊问题。

---

## 2. 三层职责边界

```
┌─────────────────────────────────────────────────────────────┐
│  Hooks 层（强制守卫）                                          │
│  职责：阻断决策、阶段门控、预算保护                              │
│  做得好：verdict 强制阻断、exit code 注入、引擎级拦截            │
│  不该做：文件 IO、状态查询、技能加载                             │
├─────────────────────────────────────────────────────────────┤
│  MCP 层（能力执行）                                            │
│  职责：文件编辑、状态暴露、工具调用、跨引擎接口统一               │
│  做得好：编码保留、Checkpoint、Resource 订阅、多引擎统一接口      │
│  不该做：强制阻断引擎行为、承载 workflow 业务逻辑                │
├─────────────────────────────────────────────────────────────┤
│  Skills 层（行为引导）                                         │
│  职责：约束加载、工作流程指导、角色行为规范、上下文注入            │
│  做得好：渐进式加载、persona 隔离、prompt 工程、场景化约束        │
│  不该做：文件 IO、系统命令执行、verdict 决策                    │
└─────────────────────────────────────────────────────────────┘
```

**核心约定（不可违反）：**
- Hooks 只做 **verdict**，不做 IO
- MCP 只做**执行**，不做强制阻断
- Skills 只做**引导**，不做系统调用

---

## 3. 标准协作流程

以「AI 引擎修改一个文件」为例：

```
1. Skills 层（事前）
   SessionStart → sy-workflow skill 加载约束
   → persona 绑定确认 → 上下文注入完成

2. Hooks 层（守卫）
   PreToolUse:Write → hook-client.cjs
   → 检查文件类别（file-classes.yaml）
   → 检查 persona 写权限
   → 返回 verdict（allow / block / block_with_approval_request）
   ↓ allow

3. MCP 层（执行）
   → read_file（编码检测 + cache）
   → edit（三级匹配 + Checkpoint）
   → verify_syntax（< 5ms 语法校验）
   → PostToolUse:Write → sy_posttool_write（证据记录 → journal）
```

---

## 4. Hook Verdict → MCP 工具联动

| Hook verdict | MCP 层响应 | 说明 |
|---|---|---|
| `allow` | 继续执行 write/edit | 正常路径 |
| `block` | 不调用任何 MCP 写工具 | hook 已拒绝，直接返回错误给引擎 |
| `block_with_approval_request` | 挂起，等待 `sy_advance_node` approval | approval_pending 状态 |
| `notify_only` | 执行并记录 `sy_posttool_write` 证据 | 低风险变更，仅通知 |

---

## 5. Skill 加载时机规范

| 时机 | 必须加载的 Skill | 触发方式 |
|---|---|---|
| `SessionStart` | `sy-workflow`（基础约束）、`sy-constraints`（基线约束） | `sy_session_start` MCP 工具 |
| 节点进入 | 当前节点对应的 task-specific skill（最多 2 个） | `sy_advance_node` 后按需加载 |
| 安全升级 | 立即加载全量安全约束 skill | hook incident 触发 |

**原则**：基线 + 最多 2 个 task-specific，避免上下文膨胀。

---

## 6. Loop Budget 工具层检查规范

来源：`refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md` §2.2

`sy_pretool_bash` 和 `sy_advance_node` 执行前须检查以下六项指标：

| 指标 | 字段 | 超限行为 |
|---|---|---|
| 节点数上限 | `max_nodes` / `consumed_nodes` | 返回 `block` + `budget_exceeded: nodes` |
| 时间上限 | `max_minutes` | 返回 `block` + `budget_exceeded: time` |
| 失败次数 | `max_failures` / `consumed_failures` | 返回 `block` + `budget_exceeded: failures` |
| 待审批上限 | `max_pending_approvals` | 返回 `block` + `budget_exceeded: approvals` |
| 上下文利用率 | `max_context_utilization` | 返回 `block` + `budget_exceeded: context` |
| 返工周期 | `max_rework_cycles` / `consumed_rework_cycles` | 返回 `block` + `budget_exceeded: rework` |

**超限响应格式：**
```json
{
  "verdict": "block",
  "budget_exceeded": "nodes",
  "detail": "consumed_nodes(21) >= max_nodes(20)"
}
```

---

## 7. Crash Recovery 协议

来源：`refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md` §2.2

`sy_session_start` 工具执行流程：

```
1. 读取 session.yaml + journal.jsonl
2. 扫描孤儿事件：有 tool_request 无 tool_completion
   → 自动补写 aborted 记录
3. 按 TDD 状态机决定恢复点：
   - red_pending  → 保持 red_pending（测试尚未通过）
   - red_verified → 可继续 green_pending
   - green_verified → 可继续 refactor_pending
4. 返回恢复后的完整状态
```

**skip_recovery=true** 时跳过步骤 2-3，直接返回当前状态（用于干净启动）。

---

## 8. 统一证据链格式（evidence-chain）

所有 `sy_posttool_write` 写入 `journal.jsonl` 的证据条目须遵循此格式：

```json
{
  "ts": "2026-03-17T10:00:00Z",
  "event": "tool_completion",
  "tool": "edit",
  "path": "seeyue-mcp/src/tools/edit.rs",
  "lines_changed": 12,
  "outcome": "success",
  "checkpoint_label": "pre-edit-20260317",
  "syntax_valid": true,
  "node_id": "n-03"
}
```

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `ts` | ISO8601 | ✓ | 事件时间戳 |
| `event` | enum | ✓ | `tool_request` / `tool_completion` / `aborted` |
| `tool` | string | ✓ | 工具名称 |
| `path` | string | ✓ | 受影响文件路径（workspace 相对路径）|
| `lines_changed` | int | - | 变更行数（edit/write 时填写）|
| `outcome` | enum | ✓ | `success` / `failure` / `aborted` |
| `checkpoint_label` | string | - | 关联的 checkpoint label |
| `syntax_valid` | bool | - | verify_syntax 结果（仅代码文件）|
| `node_id` | string | - | 当前 workflow 节点 ID |

---

## 9. 跨引擎行为一致性要求

三引擎（Claude Code / Gemini CLI / Codex）通过同一 MCP 接口操作时：

1. Hook verdict 结果必须一致（同一文件类别 → 同一 verdict）
2. Skill 约束效果必须一致（同一 persona → 同一约束集）
3. 证据链格式必须一致（同一 journal schema）

验证方式：`run_test` 执行跨引擎集成测试套件，比对 verdict + journal 输出。
