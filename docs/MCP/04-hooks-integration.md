# Hooks 系统 MCP 化方案

> 来源：`workflow/hooks.spec.yaml`，`scripts/runtime/hook-client.cjs`
> 来源：`docs/hooks-architecture-design.md`，`docs/seeyue-workflows-mcp-integration-windows.md` §第三部分
> 参考：`refer/agent-source-code/gemini-cli-main/`（四层架构），`refer/agent-source-code/claude-code-main/`（hooks 事件）

---

## 1. 现有 Hooks 架构

**当前 Hook 事件清单**（来源：`workflow/hooks.spec.yaml` event_matrix，逐字核实）：

| 事件 | 目的 | 引擎支持 |
|------|------|----------|
| `SessionStart` | bootstrap routing and constraints | claude_code: supported, codex: bridged, gemini_cli: supported |
| `UserPromptSubmit` | long-session prompt re-anchor | claude_code: supported, codex: bridged, gemini_cli: supported |
| `PreToolUse:Bash` | command class, approval, and destructive guard | claude_code: supported, codex: bridged, gemini_cli: supported |
| `PreToolUse:Write\|Edit` | TDD, secret, protected-file, and debug gates | claude_code: supported, codex: bridged, gemini_cli: supported |
| `PostToolUse:Write\|Edit` | write evidence and scope drift capture | claude_code: supported, codex: bridged, gemini_cli: supported |
| `PostToolUse:Bash` | verification and TDD evidence capture | claude_code: supported, codex: bridged, gemini_cli: supported |
| `Stop` | checkpoint and resume-frontier gate | claude_code: supported, codex: bridged, gemini_cli: supported |

**运行时约束**（来源：`workflow/hooks.spec.yaml` runtime_constraints）：
- `stdout_json_only: true` — stdout 只输出 JSON
- `stdin_single_read: true` — stdin 单次读取
- `session_start_nonblocking: true` — SessionStart 不阻塞
- `stop_force_continue_exit_code: 0` — Stop 强制继续时退出码为 0

---

## 2. Verdict 枚举（来源：`workflow/hooks.spec.yaml` verdicts，逐字核实）

```
allow                    — 放行，继续执行
block                    — 阻断，不执行
block_with_approval_request — 阻断并请求人工审批
force_continue           — 强制继续（绕过 Stop 阻断）
```

**注意**：verdict 无 `ask`、`deny` 字段，不可混用。
`block_with_approval_request` 对应 `session.approvals.pending = true`，需等待人工确认后继续。

**⚠️ Stop hook 语义特别说明（来源：`refer/skills-and-hooks-architecture-advisory.md` §2.2 盲点7）**

`Stop` hook 返回 `force_continue`（Claude Code 对应 `{ "decision": "block" }`）的语义是：**强制 Agent 继续工作，而非报错阻断**。

```
✅ 正确理解：Stop hook 阻断的是「停止」，不是「前进」
   force_continue → Agent 继续执行，不结束会话

❌ 错误理解：force_continue 是错误信号，代表"阻止继续"
```

典型用途：resume-frontier 未满足时，`sy_stop` 返回 `force_continue`，驱动 Agent 继续处理未完成节点，而不是抛出错误。

---

## 3. MCP 化目标架构

来源：`docs/hooks-architecture-design.md` §1.3，§第二章

```
┌──────────────────────────────────────────────────┐
│  AI 引擎（Claude Code / Gemini CLI / Cursor）     │
│  触发 Hook 事件 → 调用 MCP Tools                 │
└──────────────────────┬───────────────────────────┘
                       │ MCP stdio
┌──────────────────────▼───────────────────────────┐
│  seeyue-mcp（Rust）                               │
│                                                  │
│  Tools:                                          │
│    sy_pretool_bash(command)   → verdict          │
│    sy_pretool_write(path)     → verdict          │
│    sy_posttool_write(path)    → evidence         │
│    sy_posttool_bash(output)   → evidence         │
│    sy_stop(phase)             → gate_result      │
│    sy_session_start()         → bootstrap        │
│    sy_user_prompt()           → anchor_refresh   │
└──────────────────────┬───────────────────────────┘
                       │ IPC（JSON）
┌──────────────────────▼───────────────────────────┐
│  Node.js 运行时                                   │
│  hook-client.cjs → policy.cjs → router.cjs       │
└──────────────────────────────────────────────────┘
```

---

## 4. 四层分离架构

来源：`docs/hooks-architecture-design.md` §1.1，借鉴 `refer/agent-source-code/gemini-cli-main/packages/core/src/hooks/`

```
scripts/runtime/
├── hook-registry.cjs      # 注册层：从多源加载 hooks
│                          # 优先级：Runtime > Project > User > System
├── hook-planner.cjs       # 计划层：事件 + 上下文 → 执行计划
├── hook-runner.cjs        # 执行层：调用 hook 脚本，处理超时
└── hook-aggregator.cjs    # 聚合层：合并结果，应用事件策略
```

**参考源码**：
- `gemini-cli-main/packages/core/src/hooks/hookSystem.ts`（行 45-120）
- `gemini-cli-main/packages/core/src/hooks/hookPlanner.ts`（行 30-85）

---

## 5. MCP Tool 规格

### 5.1 sy_pretool_bash

```json
{
  "name": "sy_pretool_bash",
  "description": "Evaluate a bash command against workflow policy before execution",
  "inputSchema": {
    "type": "object",
    "properties": {
      "command": { "type": "string", "description": "The bash command to evaluate" },
      "cwd":     { "type": "string", "description": "Working directory" }
    },
    "required": ["command"]
  }
}
```

**返回值**（verdict 枚举与 `workflow/hooks.spec.yaml` 对齐）：
```json
{
  "verdict": "allow" | "block" | "block_with_approval_request" | "force_continue",
  "reason": "string",
  "instructions": ["string"]
}
```

现有对应实现：`scripts/runtime/hook-client.cjs` `handlePretoolBash()`

### 5.2 sy_pretool_write

```json
{
  "name": "sy_pretool_write",
  "inputSchema": {
    "type": "object",
    "properties": {
      "path":        { "type": "string" },
      "change_class": { "type": "string", "description": "docs|scaffold|utility|behavior_change|..." }
    },
    "required": ["path"]
  }
}
```

守卫项（来源：`workflow/hooks.spec.yaml` PreToolUse:Write|Edit）：
1. TDD 红灯守卫
2. Secret 文件检测
3. 保护文件拦截（`workflow/file-classes.yaml` system_file / security_boundary 等）
4. Session 完整性检查
5. Persona 写权限检查（来自 Batch 2.2：`hook-client.cjs` `checkPersonaWritePermission`）

**低价值告警豁免清单（来源：`refer/agent-source-code/claude-code-security-review-main/claudecode/findings_filter.py` HardExclusionRules）**

以下类型的 block 请求应直接降级为 `allow`，不触发 `block_with_approval_request`，避免 approval 噪音：

| 告警类型 | 说明 |
|----------|------|
| DoS / 资源耗尽 / 无限循环 | 理论性风险，非实际漏洞 |
| Rate limiting 缺失 | 架构建议，非写入守卫范畴 |
| Resource leak / Memory safety | 静态分析工具负责，hook 不处理 |
| Open redirect | 需结合运行时上下文，静态守卫误报率高 |
| 配置文件中的占位 secret（`example`/`placeholder`/`dummy`） | 非真实密钥 |
| 纯理论性问题（无具体证据） | 置信度 < 80%，不值得 approval 中断 |

**⚠️ 两阶段过滤架构（来源：claude-code-security-review-main）**

`sy_pretool_write` 的守卫决策应遵守两阶段模式：

```
阶段 1（硬规则）：正则匹配豁免清单 → 直接 allow（零延迟）
阶段 2（语义判断）：剩余 findings 查询 policy kernel → 返回 verdict
```

避免把所有低置信度告警都发 approval，造成 Agent 被频繁打断。

**返回值**：
```json
{
  "verdict": "allow" | "block" | "block_with_approval_request",
  "reason": "string",
  "normalized_input": {
    "path": "string (可选：规范化后的路径，hook 可纠正路径格式漂移而无需 block)"
  }
}
```

**⚠️ normalized_input 设计说明（来源：`refer/skills-and-hooks-architecture-advisory.md` §2.2 盲点6）**

Claude Code `PreToolUse` hook 支持在返回时 **rewrite `tool_input`**，即在不 block 的情况下纠正参数漂移。`normalized_input` 字段对应此能力：

```
典型场景：Agent 传入 Linux 风格路径 src/auth/jwt.rs
hook 不 block，而是返回 normalized_input.path = "src\\auth\\jwt.rs"
等效于调用 resolve_path 工具，但在守卫层透明完成，减少 approval 中断
```

`normalized_input` 为可选字段，不填时行为与当前一致。

```json
{
  "name": "sy_posttool_write",
  "inputSchema": {
    "type": "object",
    "properties": {
      "path":    { "type": "string" },
      "outcome": { "type": "string", "enum": ["success", "error"] }
    },
    "required": ["path", "outcome"]
  }
}
```

现有对应实现：`scripts/runtime/hook-client.cjs` `handlePosttoolWrite()`
捕获写入证据，检测 scope 漂移，写入 `journal.jsonl`。

**⚠️ journal.jsonl 原子写入约束（来源：`refer/skills-and-hooks-architecture-advisory.md` §1.4，§2.2 盲点3）**

Claude Code 可以并行触发多个 `PostToolUse` hook（subagent 模式下尤为常见），因此写入 `journal.jsonl` 和 `session.yaml` 必须使用原子重命名，不得直接 append：

```
✅ 正确：write-to-tmp → fsync → rename（原子操作）
❌ 错误：fs.appendFile / 直接 std::fs::write（并发时会产生写入竞态）
```

具体实现：`sy_posttool_write` 在 Rust 侧写 journal 时，必须先写入临时文件（同目录，`journal.{pid}.tmp`），再调用 `std::fs::rename` 原子替换。`session.yaml` 更新同理。

### 5.4 sy_posttool_bash

捕获验证和 TDD 证据（红/绿状态），写入 `journal.jsonl`。

现有对应实现：`scripts/runtime/hook-client.cjs` `handlePosttoolBash()`

### 5.5 sy_stop

```json
{
  "name": "sy_stop",
  "inputSchema": {
    "type": "object",
    "properties": {
      "phase":   { "type": "string" },
      "node_id": { "type": "string" }
    }
  }
}
```

检查项：checkpoint 创建、恢复边界、完整收口验证。
返回值 verdict 可为 `allow`（正常收口）或 `force_continue`（继续执行）。

> `force_continue` 的正确语义：强制 Agent **继续工作**，驱动其处理未完成节点，而不是报错停止。详见 §2 verdict 语义说明。

---

## 6. 三大硬约束的 MCP 映射

来源：`docs/hooks-architecture-design.md` §1.1

| 硬约束 | 现有实现 | MCP Tool |
|--------|---------|----------|
| 防止阶段越界 | `phase-guard.cjs` | `sy_pretool_bash`（classifyCommand + phaseCheck）|
| 防止危险写入 | `hook-client.cjs` handlePretoolWrite | `sy_pretool_write` |
| 确保完整收口 | `hook-client.cjs` handleStop | `sy_stop` |

**⚠️ Hook 与 Policy Kernel 必须保持独立（来源：`refer/skills-and-hooks-architecture-advisory.md` §1.5，§2.2 盲点5）**

行业收敛结论：「Hooks enforce, Policy Kernel decides」是所有主流框架（Claude Code / Gemini CLI / LangChain）共同验证的正确模式。

```
 Policy Kernel（决策）          Hook Script（执行边界）
 ─────────────────────          ────────────────────────
 policy.cjs                     hook-client.cjs
 读取 spec → 返回 verdict        调用 Policy Kernel → 按 verdict 阻断
 纯函数，无副作用                有副作用（写 journal、触发 approval）
```

**Gemini CLI 专项说明**：Gemini 有独立的 Policy Engine（TOML rules），正确的 Gemini adapter 应将 read-only allow 规则和 admin-tier TDD hard block 编译为 `.gemini/policies/v4-workflow.toml`，hook script 只做证据记录和状态同步。若 Gemini adapter 仅安装 hook script，每次工具调用都需 shell 进程开销，性能差且语义不匹配。这对应 `02-architecture.md` §8 三层协作插槽中的 Gemini 专项扩展点。

---

## 7. 决策聚合模型

来源：`workflow/hooks.spec.yaml` verdicts
来源（真实代码验证）：`refer/agent-source-code/gemini-cli-main/packages/core/src/hooks/hookAggregator.ts`

```javascript
// hook-aggregator.cjs（目标架构）
// 优先级：block > block_with_approval_request > allow
function aggregateDecisions(hookResults) {
  if (hookResults.some(r => r.verdict === 'block')) {
    return { verdict: 'block', reason: '...' };
  }
  if (hookResults.some(r => r.verdict === 'block_with_approval_request')) {
    return { verdict: 'block_with_approval_request', reason: '...' };
  }
  return { verdict: 'allow' };
}
```

`force_continue` 仅用于 Stop 事件，优先级独立。

**源码验证（hookAggregator.ts `mergeWithOrDecision`）**：Gemini CLI 对 BeforeTool/AfterTool/SessionStart 使用 OR 逻辑——任一 hook 返回 blocking decision 则整批结果为 block。`sy_pretool_bash` / `sy_pretool_write` 多 hook 场景必须遵守同样的 OR 聚合语义。

**Gemini HookDecision → seeyue verdict 映射表**（来源：`hookAggregator.ts` types.ts）：

| Gemini HookDecision | seeyue verdict | 说明 |
|---------------------|----------------|------|
| `'block'` / `'deny'` | `block` | 硬阻断 |
| `'ask'` | `block_with_approval_request` | 请求用户确认 |
| `'approve'` / `'allow'` / `undefined` | `allow` | 放行 |
| （Stop 事件专用）| `force_continue` | 强制继续工作 |

Gemini adapter 在调用 seeyue-mcp 工具时，必须将 `HookDecision` 转换为对应的 `verdict`，转换逻辑在 compile-adapter.cjs 中实现。

---

## 7.1 Policy Engine 规则执行语义

来源（真实代码验证）：`refer/agent-source-code/gemini-cli-main/packages/core/src/policy/policy-engine.ts`

```typescript
// PolicyEngine 构造时对规则按 priority 降序排列
this.rules = (config.rules ?? []).sort(
  (a, b) => (b.priority ?? 0) - (a.priority ?? 0)
);
// 执行时：first_match_wins（第一条匹配的规则决定结果）
```

`policy.cjs` 必须遵守相同语义：
- 规则按 `priority` 字段降序排列（数值越大优先级越高）
- 第一条匹配的规则决定 verdict，不继续评估后续规则
- admin-tier 规则（最高 priority）作为硬约束覆盖所有低优先级规则

**规则匹配维度**（来源：`ruleMatches()` 函数）：
1. `modes`：按 ApprovalMode（DEFAULT/AUTO_EDIT/YOLO/PLAN）过滤
2. `toolName`：支持 `*`、`mcp_serverName_*` 通配符
3. `toolAnnotations`：工具注解键值对匹配
4. `argsPattern`：对 JSON 序列化参数做正则匹配

**PolicyDecision 三值**（Gemini 真实枚举）：`ALLOW` / `DENY` / `ASK_USER`

---

## 7.2 Scheduler：Policy 先于 Confirmation

来源（真实代码验证）：`refer/agent-source-code/gemini-cli-main/packages/core/src/scheduler/scheduler.ts`

```
工具调用执行顺序（强制）：
1. checkPolicy()       ← Policy Engine 评估（纯规则，无副作用）
2. resolveConfirmation() ← 用户确认（仅当 policy 返回 ASK_USER）
3. ToolExecutor.execute() ← 实际执行
4. CompletedToolCall    ← 结果记录
```

**seeyue-mcp 约束**：`sy_pretool_bash` 和 `sy_pretool_write` 内部调用 `policy.cjs` 的时序必须在 `approval_pending` 触发之前完成。不允许先触发 approval 再做 policy 评估。

---

## 8. 向后兼容策略

MCP Tools 是**新增接口**，不替换现有 `hook-client.cjs` 逻辑：

```
阶段 1（P0）：MCP Server 实现，内部调用现有 Node.js hook-client.cjs
  seeyue-mcp sy_pretool_bash → IPC → hook-client.cjs handlePretoolBash()

阶段 2（P1）：重构 hook-client.cjs 为四层分离架构
  seeyue-mcp sy_pretool_bash → IPC → hook-runner.cjs

阶段 3（P2）：Rust 原生实现高频 hooks（跳过 IPC）
  seeyue-mcp sy_pretool_bash → Rust 直接实现（最低延迟）
```

---

## 9. MCP 相比现有 Hooks 的明确优势场景

> 分析来源：架构对比，基于 `docs/MCP/02-architecture.md`、`docs/MCP/03-file-editing-engine.md`、`workflow/hooks.spec.yaml`

### 9.1 文件编辑操作（PreToolUse:Write|Edit 守卫对象）

**现有问题**：AI 引擎用自身原生 Write/Edit 工具写文件，`hook-client.cjs` 只在写入**前**做守卫决策，无法介入编辑过程本身（Tab/CRLF/GBK 保留、三级匹配 fallback、Checkpoint 快照均不存在）。

**MCP 优势**：`read_file → edit → verify_syntax` 构成完整闭环——
- Tab 保留为 `\t`（杜绝 Claude Code Issue #26996）
- 编码往返校验（UTF-8 / GBK / Shift-JIS / UTF-16LE 均保留）
- 三级匹配 fallback（精确 → Tab/Space 规范化 → Unicode 混淆检测）
- Checkpoint SQLite WAL 快照，`rewind` 可撤销任意步
- `verify_syntax` 写入前 < 5ms 语法校验，阻止写入语法错误内容

写入质量从源头保证，无需 hook 事后补救。

**性能对比**：

| 维度 | 现有 Node.js hook | MCP Rust Server |
|------|------------------|-----------------|
| 启动延迟 | 300–800ms（Node.js 初始化）| < 30ms（单进程常驻）|
| 内存占用 | ~80MB | < 8MB（no GC）|
| Windows Defender | 每次启动扫描 node_modules | 一次扫描后缓存 |
| 编码处理 | 无（依赖引擎） | chardetng + encoding_rs 完整覆盖 |

---

### 9.2 工作区状态读取（SessionStart / UserPromptSubmit）

**现有问题**：每次 `SessionStart` / `UserPromptSubmit` hook 触发都要启动 Node.js 子进程，读取 `.ai/workflow/session.yaml`，冷启动延迟 300ms+，长会话中累积显著。

**MCP 优势**：三个 Workflow Resources 长驻 Rust 进程内存：

```
workflow://session    → 直接文件读取（< 1ms）
workflow://task-graph → 直接文件读取（< 1ms）
workflow://journal    → 直接文件读取（< 1ms）
```

引擎通过 `resources/subscribe` 订阅变更通知，`sy_advance_node` 触发后自动推送，无需 hook 轮询，也无需重新启动子进程。

---

### 9.3 多引擎统一接口（bridged 模式消除）

**现有问题**（来源：`workflow/hooks.spec.yaml` event_matrix）：

```
Claude Code：claude_code: supported  — 原生 hooks 协议
Codex：      codex: bridged          — 需要独立桥接适配层
Gemini CLI： gemini_cli: supported   — 独立实现，维护分叉
```

三套接入逻辑需分别维护，`codex: bridged` 模式额外引入转换开销。

**MCP 优势**：三个引擎通过同一个 `seeyue-mcp.exe` stdio 接口（JSON-RPC 2.0）访问所有能力，协议层完全统一：

```
Claude Code → seeyue-mcp.exe (stdio)
Codex       → seeyue-mcp.exe (stdio)  ← 消除 bridged 适配层
Gemini CLI  → seeyue-mcp.exe (stdio)
```

MCP 服务端只需维护一份实现，引擎差异由 MCP 协议屏蔽。

---

### 9.4 不适合 MCP 替换的功能（边界说明）

以下功能**必须保留现有 hook 机制**，MCP 工具无法替代：

| 功能 | 原因 |
|------|------|
| `PreToolUse:Bash` 强制阻断 | hook stdout verdict 是引擎**强制**阻断协议；MCP Tool 是主动调用，无法拦截引擎原生 Bash 工具的执行 |
| `Stop` resume-frontier 门控 | 需在引擎停止**前**通过 exit code 注入，MCP 无法感知引擎停止事件 |
| verdict `block` 决策权 | 引擎只读取 hook stdout JSON 中的 verdict 作为阻断依据，MCP 返回值对引擎原生工具调用没有强制约束力 |

**混合最优模式**（与 §8 向后兼容策略对应）：

```
引擎原生 Hook（保留阻断权）     MCP Tool（承担实际执行）
────────────────────────       ──────────────────────────
PreToolUse:Bash  → verdict     sy_pretool_bash（策略查询）
PreToolUse:Write → verdict     read_file / edit / verify_syntax（写入执行）
Stop             → gate        workflow://session（状态读取）
SessionStart     → bootstrap   sy_session_start（快速初始化）
```

`hook-client.cjs` 继续作为**强制阻断层**，MCP 承担**能力执行层**，两者互补而非替代。
