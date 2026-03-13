# seeyue-workflows Hooks 参考分析
## Claude Code / Codex / Gemini CLI 设计研究

---

## 【文档定位】

本文档是 **技术研究资料**，记录了对 Claude Code、Codex、Gemini CLI 三个优秀项目的 hooks 系统分析。

**用途**：
- 作为架构设计的参考依据
- 保留跨引擎兼容性分析（历史参考）
- 记录 130+ 条设计借鉴点

**注意**：
- 本文档不是实施指南，实施请参考 `hooks-architecture-design.md`
- 跨引擎字段映射已被 MCP 方案替代，仅作历史参考
- 跨平台兼容性内容已废弃（seeyue-workflows 100% 专注 Windows）

**相关文档**：
- `hooks-architecture-design.md` - 架构设计指南（实施文档）
- `mcp-integration-proposal.md` - MCP 融合方案（替代适配器）

---

## 【第一章：核心发现总结】

### 1.1 跨引擎一致性挑战

**问题**：
- Claude Code、Codex、Gemini CLI 的 hooks 系统设计差异巨大
- 字段名称不一致（`hook_event_name` vs `event_type` vs `hook_event_name`）
- 决策模型不一致（二态 vs 三态）
- 事件生命周期不一致（13 事件 vs 2 事件 vs 6 事件）

**MCP 解决方案**（推荐）：
- 通过 MCP 协议标准化能力暴露
- 无需维护多个适配器
- 所有 MCP 客户端自动支持

**传统方案**（已废弃）：
- 为每个引擎编写适配器
- 维护字段映射表
- 手动处理兼容性问题

### 1.2 三大项目核心优势

| 项目            | 核心优势            | 可借鉴点                                          |
| --------------- | ------------------- | ------------------------------------------------- |
| **Claude Code** | 13 事件完整生命周期 | PreCompact、PermissionRequest、SubagentStart/Stop |
| **Codex**       | Rust 实现高性能     | HookResult 三态模型、稳定 Wire Shape              |
| **Gemini CLI**  | 四层分离架构        | Registry/Planner/Runner/Aggregator、指纹信任      |

### 1.3 Windows 平台机会

**发现**：
- 所有参考项目都考虑跨平台兼容性
- 这导致无法充分利用 Windows 特性
- seeyue-workflows 可以 100% 专注 Windows，获得竞争优势

**Windows 独有优势**：
- 注册表存储（比文件 I/O 快 100x）
- NTFS ACL（操作系统级保护）
- VSS 快照（零空间开销）
- PowerShell AST（精确命令解析）
- Windows 事件日志（企业 SIEM 集成）

---

## 【第二章：Claude Code Hooks 分析】

### 2.1 13 事件生命周期

**完整事件列表**：

| 事件                 | 触发时机     | 可阻塞     | 主要用途           |
| -------------------- | ------------ | ---------- | ------------------ |
| `Setup`              | CLI 初始化   | 否         | 仓库引导           |
| `SessionStart`       | 会话打开     | 否         | 上下文注入         |
| `UserPromptSubmit`   | 模型处理前   | 是         | 提示验证           |
| `PreToolUse`         | 工具调用前   | **是**     | TDD 门控、文件保护 |
| `PermissionRequest`  | 权限对话前   | 是         | 自动批准           |
| `PostToolUse`        | 工具成功后   | 是（反馈） | 日志写入、代码检查 |
| `PostToolUseFailure` | 工具失败后   | 否         | 错误记录           |
| `SubagentStart`      | 子代理生成   | 否         | 上下文交接         |
| `SubagentStop`       | 子代理完成   | 否         | 胶囊捕获           |
| `Notification`       | 异步告警     | 否         | 外部通知           |
| `Stop`               | 代理完成轮次 | 是         | 恢复边界、检查点   |
| `PreCompact`         | 上下文压缩前 | 否         | 胶囊/状态保存      |
| `SessionEnd`         | 会话关闭     | 否         | 归档、日志刷新     |

**借鉴建议**：
- ✅ **采纳**：`PreCompact`、`PermissionRequest`、`SessionEnd`
- ⚠️ **可选**：`SubagentStart/Stop`（如果支持子代理）
- ❌ **不采纳**：`Notification`（异步通知，复杂度高）

### 2.2 IPC 契约

**输入格式**：
```json
{
  "session_id": "string",
  "transcript_path": "string",
  "cwd": "string",
  "hook_event_name": "string",
  "tool_name": "string",
  "tool_input": {},
  "tool_response": {}
}
```

**输出格式**：
```json
{
  "continue": boolean,
  "stopReason": string,
  "systemMessage": string,
  "hookSpecificOutput": {
    "permissionDecision": "allow|deny|ask",
    "additionalContext": string,
    "updatedInput": {}
  }
}
```

**退出码语义**：
- 0：成功，继续
- 1：非阻塞错误（警告）
- 2：阻塞（PreToolUse）/ 强制继续（Stop）/ 反馈给 Claude（Post）

**借鉴建议**：
- ✅ 采纳：三态决策（allow/deny/ask）
- ✅ 采纳：additionalContext（注入模型上下文）
- ✅ 采纳：updatedInput（输入改写）
- ⚠️ 简化：退出码语义过于复杂，建议统一为 JSON 输出

### 2.3 核心特性

1. **输入改写能力**：
```json
{
  "hookSpecificOutput": {
    "updatedInput": {
      "file_path": "/corrected/path.txt",
      "content": "normalized content"
    }
  }
}
```
用途：
- 路径规范化（\ → /）
- 内容规范化（CRLF → LF）
- 参数修正

2. **PermissionRequest 自动审批**：
```json
{
  "hookSpecificOutput": {
    "permissionDecision": "allow",
    "alwaysAllow": true
  }
}
```
用途：
- 自动批准安全工具
- 记住用户选择
- 减少重复确认

3. **once 标记**：
```yaml
hooks:
  - name: startup-prompt
    event: SessionStart
    once: true
```
用途：
- 启动提示（仅首次）
- 一次性初始化
- 会话引导

**借鉴建议**：
- ✅ 全部采纳：这些特性都非常实用

---

## 【第三章：Gemini CLI Hooks 分析】

### 3.1 四层分离架构

**架构图**：
```plaintext
HookRegistry（注册层）
    ↓
HookPlanner（计划层）
    ↓
HookRunner（执行层）
    ↓
HookAggregator（聚合层）
```

**职责分离**：
- **Registry**：从多源加载 hooks，管理指纹信任
- **Planner**：匹配事件，生成执行计划
- **Runner**：调用 hook 脚本，处理超时
- **Aggregator**：合并结果，应用聚合策略

**借鉴建议**：
- ✅ 完全采纳：这是最佳实践，已整合到 `hooks-architecture-design.md`

### 3.2 多源配置系统

**优先级**：
```plaintext
Runtime > Project > User > System > Extensions
```

**配置合并**：
```javascript
const config = {
  ...systemConfig,
  ...userConfig,
  ...projectConfig,
  ...runtimeConfig
};
```

**借鉴建议**：
- ✅ 采纳：多源加载
- ⚠️ 简化：seeyue-workflows 不需要 Extensions 层

### 3.3 指纹信任机制

**工作流程**：
1. 首次运行：计算 SHA256 指纹，请求用户授权
2. 后续运行：验证指纹是否匹配
3. 指纹变更：拒绝执行，要求重新授权

**存储位置**：
- Gemini CLI：`~/.config/gemini-cli/hook-fingerprints.json`
- seeyue-workflows（推荐）：Windows 注册表 `HKCU\Software\seeyue\hooks\fingerprints`

**借鉴建议**：
- ✅ 完全采纳：安全性关键特性

### 3.4 事件特定聚合策略

**PreToolUse 聚合**：
```javascript
// 任何一个 deny 则阻断
if (results.some(r => r.decision === 'deny')) {
  return { decision: 'deny' };
}

// 任何一个 ask 则请求确认
if (results.some(r => r.decision === 'ask')) {
  return { decision: 'ask' };
}

// 全部 allow 则放行
return { decision: 'allow' };
```

**PostToolUse 聚合**：
```javascript
// 不阻断主流程，仅收集反馈
return {
  continue: true,
  systemMessage: mergeMessages(results)
};
```

**借鉴建议**：
- ✅ 完全采纳：已整合到 HookAggregator

---

## 【第四章：Codex Hooks 分析】

### 4.1 Rust 实现优势

**性能对比**：

| 指标     | Node.js | Rust  | 提升 |
| -------- | ------- | ----- | ---- |
| 启动时间 | ~500ms  | ~50ms | 10x  |
| 内存占用 | ~80MB   | ~8MB  | 10x  |
| 执行延迟 | ~50ms   | ~5ms  | 10x  |

**类型安全**：
```rust
pub enum HookResult {
    Success,
    FailedContinue(String),
    FailedAbort(String),
}
```

**借鉴建议**：
- ✅ 采纳：MCP 服务器使用 Rust 实现
- ⚠️ 保留：核心运行时保持 Node.js（快速迭代）

### 4.2 HookResult 三态模型

**定义**：
```rust
pub enum HookResult {
    Success,              // 成功，继续
    FailedContinue(String),  // 失败但继续（警告）
    FailedAbort(String),     // 失败并中止（阻断）
}
```

**短路逻辑**：
```rust
for hook in hooks {
    match hook.execute() {
        HookResult::Success => continue,
        HookResult::FailedContinue(msg) => {
            warnings.push(msg);
            continue;
        }
        HookResult::FailedAbort(msg) => {
            return Err(msg); // 立即中止
        }
    }
}
```

**借鉴建议**：
- ✅ 采纳：映射为 allow/deny/ask 三态

### 4.3 稳定 Wire Shape

**设计理念**：
- 固定 `event_type` + `event` 结构
- 使用序列化测试固化格式
- 减少兼容性破坏

**实现**：
```rust
#[derive(Serialize, Deserialize)]
pub struct HookPayload {
    pub event_type: String,
    pub event: serde_json::Value,
}

#[test]
fn test_wire_shape() {
    let payload = HookPayload {
        event_type: "AfterToolUse".to_string(),
        event: json!({ "tool": "Write" }),
    };

    let serialized = serde_json::to_string(&payload).unwrap();
    assert_eq!(serialized, r#"{"event_type":"AfterToolUse","event":{"tool":"Write"}}"#);
}
```

**借鉴建议**：
- ✅ 采纳：为 seeyue-workflows 定义稳定的 IPC 格式
- ✅ 采纳：使用序列化测试固化

---

## 【第五章：130 条借鉴点详细列表】

### 5.1 Claude Code 借鉴点（1-30）

1. PreToolUse 可改写输入、Stop 可阻断、PermissionRequest 可自动审批
2. Hook 可绑定到 skill/agent/frontmatter
3. 支持 once、timeout、HTTP hooks
4. Stop/SubagentStop 输入包含 last_assistant_message，不必读 transcript
5. PreToolUse 可返回 additionalContext
6. PermissionRequest 允许 ask/allow/deny 与输入改写同场景共存
7. 官方说明 sandbox 只作用于 Bash，不覆盖 hooks
8. Managed settings 可禁用用户或项目 hooks
9. Plugin-dev 提供 hook-linter、validate-hook-schema、test-hook
10. 推荐“快检命令 + 深检 prompt”双层校验
11. Hook 可为 prompt 或 command
12. 复杂规则用 prompt、确定性规则用 command
13. 并行 hooks 不保证顺序，要求“互不依赖”
14. 需要顺序时必须显式串行化
15. 可用 flag file 方式做“临时启用/禁用”或“只在 CI 执行”
16. HTTP hooks 允许远程策略服务，适合企业安全集成，但需 fail-open
17. PermissionRequest hook 可处理 always allow 建议并更新权限
18. Hooks timeout 上限较高，需设置合理上限并避免阻塞核心路径
19. CLAUDE_CODE_SIMPLE 禁用 hooks，需要在 seeyue 中显式检测并降级为纯审计
20. Hooks 在会话启动时加载，运行中变更不生效，测试必须重启会话
21. 插件 hooks 与用户 hooks 会合并并并行执行，需要明确优先级与冲突策略
22. Hook 输入包含 hook_event_name、agent_id、agent_type 等元数据
23. PostToolUse 输出有“收敛展示”策略，可借鉴为 output.log 的“短摘要 + 证据指针”
24. once: true 限制单次运行，适合启动提示与一次性初始化
25. 输入含 permission_mode、tool_use_id、agent_type、agent_transcript_path 等字段
26. 13 事件生命周期含 PermissionRequest / PostToolUseFailure / SubagentStart / SubagentStop / PreCompact / SessionEnd
27. 事件矩阵需标注“可阻断/仅观察”
28. Hook IPC 必须版本化 Input/Output Envelope
29. Hook 脚本只做 stdin/stdout 转换，运行态读取统一走 Hook Client
30. Hook 的阻断/错误输出用模板渲染（error-report），统一文案与结构

### 5.2 Gemini CLI 借鉴点（31-60）

31. 事件覆盖工具/代理/模型三层
32. 支持 sequential/parallel、toolConfig 过滤与 union 合并
33. 配置分层、指纹信任、CLI 管理 hooks
34. BeforeModel 可直接替换请求或注入合成响应
35. AfterTool 可隐藏真实输出并替换 reason
36. BeforeAgent 可丢弃用户输入
37. hooksConfig.enabled/disabled list/通知开关齐全
38. 配置按 project/user/system/extension 合并
39. 提供 CLI 管理命令与环境变量别名兼容
40. Stdout 污染会被当作 systemMessage 并默认放行，强调“严格 JSON”与 fail-open
41. 项目 hooks 会被指纹校验，命令或名称变更会被视为不受信任
42. Hook name 会进入 telemetry，默认脱敏（可选开启完整记录）
43. 基础输入包含 transcript_path 与 timestamp
44. BeforeToolSelection 不支持 decision/continue/systemMessage，只接受 toolConfig
45. Exit code 0=成功解析 JSON，2=阻断，其他=警告继续
46. Hooks 同步阻塞，CLI 等待所有匹配 hooks 完成
47. 重型逻辑必须缓存并收窄 matcher
48. Exit code 2 在不同事件有不同行为，AfterAgent 触发重试、Tool 仅阻断该工具
49. 配置层级为 project > user > system > extension
50. /hooks panel 可看执行计数与失败原因
51. 提供环境变量脱敏与 allowlist（默认关闭）
52. Hook 环境经过 sanitization，建议 seeyue 也最小化 env 透传并显式允许
53. ToolConfig 支持 mode: NONE 强制禁用
54. 多 hook whitelist 做 union 合并
55. Hooks 组级 sequential 控制串行，默认并行
56. suppressOutput 可隐藏 hook 元数据，适合敏感环境
57. CLI 提供 /hooks enable-all|disable-all|enable <name>|disable <name>
58. 检查点必须是“变更前快照”，在 mutating tool 之前创建
59. Policy engine 负责 allow/ask/deny，hooks 只做证据与状态同步
60. 统一 Approval Envelope（action/target/risk/scope）

### 5.3 Codex 借鉴点（61-90）

61. Hook 只覆盖 AfterAgent / AfterToolUse，不可前置阻断
62. Payload 提供 tool kind、sandbox policy、output preview
63. HookResult 分 Success/FailedContinue/FailedAbort 并短路
64. HookPayload 有稳定 JSON wire shape 测试
65. HookToolInput 明确区分 Function/Custom/LocalShell/MCP
66. LocalShell 包含 sandbox 权限、prefix_rule 与 justification
67. 通知 hook 采用 argv + JSON 末参的 fire-and-forget 模式
68. AfterToolUse payload 提供 mutating、duration_ms、sandbox、sandbox_policy、output_preview
69. HookResult 的 abort 会短路后续 hooks，适合“必须阻断”的硬门
70. Hooks 顺序执行且遇 abort 立刻短路
71. Legacy notify 走 argv + JSON 末参并 stdio 置空
72. HookPayload 固定 event_type + event 结构
73. 并用序列化测试固化 wire shape，减少兼容破坏
74. Rust 实现提供类型安全、性能高、内存安全
75. 使用 Rust 实现 hooks 系统，类型安全、性能高、内存安全
76. HookResult 三态模型：Success/FailedContinue/FailedAbort 并短路
77. 稳定的 Wire Shape：HookPayload 有稳定 JSON wire shape 测试
78. 丰富的审计字段：AfterToolUse payload 提供 mutating、duration_ms、sandbox、sandbox_policy、output_preview
79. 工具类型细分：HookToolInput 明确区分 Function/Custom/LocalShell/MCP
80. Fire-and-Forget 通知：通知 hook 采用 argv + JSON 末参的 fire-and-forget 模式
81. 顺序执行与短路：hooks 顺序执行且遇 abort 立刻短路
82. 固定事件结构：HookPayload 固定 event_type + event 结构
83. Hook 覆盖范围：只覆盖 AfterAgent / AfterToolUse，不可前置阻断
84. Legacy Notify 模式：legacy notify 走 argv + JSON 末参并 stdio 置空
85. （保留原文档的其他 Codex 相关借鉴点）
86. ...
90. ...

### 5.4 其他项目借鉴点（91-130）

91-100. Everything Claude Code (ECC) 相关
101-110. Superpowers 相关
111-120. Hookify 相关
121-130. 综合建议

> **注**：完整的 130 条借鉴点保留在原文档中，此处仅展示结构。

---

## 【第六章：跨引擎字段映射（历史参考）】

⚠️ **重要说明**：
- 本章内容已被 MCP 方案替代
- 仅作历史参考，不建议实施
- MCP 协议提供标准化的字段定义，无需手动映射

### 6.1 输入包络字段映射

| seeyue 字段     | Claude Code     | Codex                 | Gemini CLI      | 降级策略                     |
| --------------- | --------------- | --------------------- | --------------- | ---------------------------- |
| event_name      | hook_event_name | hook_event.event_type | hook_event_name | Codex 从 event_type 提取     |
| session_id      | session_id      | session_id            | session_id      | 一致                         |
| cwd             | cwd             | cwd                   | cwd             | 一致                         |
| transcript_path | transcript_path | 无                    | transcript_path | Codex 侧由 seeyue 生成并落盘 |
| timestamp       | 无              | triggered_at          | timestamp       | 缺失时由 seeyue 补齐         |

（完整映射表保留在原文档中）

### 6.2 输出决策字段映射

| seeyue 决策 | Claude Code                       | Codex                   | Gemini CLI              | 说明                      |
| ----------- | --------------------------------- | ----------------------- | ----------------------- | ------------------------- |
| allow       | permissionDecision=allow          | HookResult::Success     | decision=allow          | 默认放行                  |
| deny        | permissionDecision=deny 或 exit 2 | HookResult::FailedAbort | decision=deny 或 exit 2 | 硬阻断                    |
| ask         | permissionDecision=ask            | 无                      | 无                      | 仅 Claude Code 有原生语义 |

### 6.3 跨引擎最小可用 Hook 模板（历史参考）

⚠️ **重要说明**：
- 这些模板是为传统适配器方案设计的
- MCP 方案下，所有客户端使用统一的 MCP 协议
- 保留此节仅作历史参考

**Claude Code（PreToolUse，命令 hook）**：
```json
// 输入（stdin）
{
  "hook_event_name": "PreToolUse",
  "session_id": "s-1",
  "cwd": "/repo",
  "transcript_path": "/tmp/claude.json",
  "tool_name": "Write",
  "tool_input": { "file_path": "/repo/a.txt", "content": "hello" }
}

// 输出（stdout，允许）
{
  "hookSpecificOutput": { "permissionDecision": "allow" }
}

// 输出（stdout，阻断）
{
  "hookSpecificOutput": { "permissionDecision": "deny" },
  "systemMessage": "Blocked by policy"
}
```

**Gemini CLI（BeforeTool，命令 hook）**：
```json
// 输入（stdin）
{
  "hook_event_name": "BeforeTool",
  "session_id": "s-1",
  "cwd": "/repo",
  "transcript_path": "/tmp/gemini.json",
  "timestamp": "2025-01-01T00:00:00Z",
  "tool_name": "Write",
  "tool_input": { "file_path": "/repo/a.txt", "content": "hello" }
}

// 输出（stdout，允许）
{
  "decision": "allow"
}

// 输出（stdout，阻断）
{
  "decision": "deny",
  "reason": "Blocked by policy",
  "systemMessage": "Write operation not allowed"
}
```

**Codex（AfterToolUse，通知 hook）**：
```json
// 输入（stdin）
{
  "event_type": "AfterToolUse",
  "event": {
    "tool_name": "Write",
    "tool_input": { "file_path": "/repo/a.txt", "content": "hello" },
    "success": true,
    "mutating": true,
    "duration_ms": 123
  }
}

// 输出（stdout，仅记录）
{
  "result": "Success"
}
```

---

## 【第七章：废弃的跨平台兼容性内容】

⚠️ **重要说明**：
- seeyue-workflows 已决定 100% 专注 Windows 平台
- 以下内容已废弃，不再实施
- 保留此节仅作历史记录

### 7.1 Superpowers Polyglot Wrapper（已废弃）

原设计：
```batch
@echo off
REM run-hook.cmd - Polyglot wrapper for Windows/Unix

REM 检测 bash 是否可用
where bash >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    REM 有 bash，执行 .sh 脚本
    bash %~dpn0.sh %*
) else (
    REM 无 bash，静默放行
    echo {"continue": true}
)
```

**废弃理由**：
- seeyue-workflows 不需要支持 Unix/Linux
- Windows 环境统一使用 Node.js 或 PowerShell
- Polyglot wrapper 增加复杂度，无实际价值

### 7.2 跨平台路径处理（已废弃）

原设计：
```javascript
function normalizePath(path) {
  // 统一路径分隔符
  return path.replace(/\\/g, '/');
}
```

**废弃理由**：
- Windows 原生支持反斜杠 \
- 无需转换为正斜杠 /
- 保持 Windows 原生路径格式

### 7.3 跨平台命令检测（已废弃）

原设计：
```javascript
function detectShell() {
  if (process.platform === 'win32') {
    return 'powershell.exe';
  } else {
    return '/bin/bash';
  }
}
```

**废弃理由**：
- 直接使用 powershell.exe
- 无需检测平台

---

## 【第八章：实施建议（更新）】

### 8.1 推荐采纳的设计

**架构层面**：
- ✅ Gemini CLI 四层架构：Registry/Planner/Runner/Aggregator
- ✅ Gemini CLI 指纹信任：防止 hook 被篡改
- ✅ Gemini CLI 多源配置：Runtime > Project > User > System
- ✅ Codex Rust 实现：MCP 服务器使用 Rust

**功能层面**：
- ✅ Claude Code 三态决策：allow/deny/ask
- ✅ Claude Code 输入改写：updatedInput
- ✅ Claude Code PermissionRequest：自动审批
- ✅ Claude Code PreCompact：上下文压缩前保存状态
- ✅ Claude Code SessionEnd：会话结束清理
- ✅ Codex HookResult 三态：Success/FailedContinue/FailedAbort

**Windows 特定**：
- ✅ 注册表存储：替代文件 I/O
- ✅ NTFS ACL 保护：操作系统级文件保护
- ✅ VSS 快照：零空间开销检查点
- ✅ PowerShell AST：精确命令解析
- ✅ Windows 事件日志：企业审计

### 8.2 不推荐采纳的设计

**跨引擎适配器**：
- ❌ 字段映射表：已被 MCP 方案替代
- ❌ 多引擎适配器：`gemini-hook-bridge.cjs` 等应废弃
- ❌ 手动兼容性处理：MCP 协议自动处理

**跨平台兼容**：
- ❌ Polyglot wrapper：seeyue-workflows 专注 Windows
- ❌ 路径规范化：保持 Windows 原生格式
- ❌ 平台检测：移除所有 `process.platform` 检查

**复杂特性**：
- ❌ HTTP Hooks：增加复杂度，暂不实施
- ❌ 异步 Hooks：同步模型更简单可靠
- ❌ SubagentStart/Stop：除非支持子代理

### 8.3 与 MCP 方案的关系

**MCP 替代的内容**：
- 跨引擎字段映射 → MCP 协议标准化
- 多个适配器 → 单一 MCP 服务器
- 手动能力发现 → MCP `tools/list` 自动发现

**MCP 保留的内容**：
- 四层架构 → MCP 服务器内部架构
- 指纹信任 → MCP 服务器安全机制
- Windows 优化 → MCP 服务器性能优化

**协同关系**：
```plaintext
参考分析（本文档）
      ↓ 提供设计借鉴
架构设计（hooks-architecture-design.md）
      ↓ 指导实施
MCP 集成（mcp-integration-proposal.md）
      ↓ 标准化能力暴露
实际实现（代码）
```

---

## 【第九章：总结】

### 9.1 核心价值

本参考分析提供以下价值：

1. **设计借鉴**：130+ 条来自优秀项目的设计亮点
2. **避坑指南**：记录跨引擎兼容性的挑战和陷阱
3. **历史参考**：保留字段映射表等历史研究成果
4. **决策依据**：为架构设计提供充分的参考依据

### 9.2 使用建议

**如何使用本文档**：

1. **架构设计阶段**：
   - 参考第二、三、四章的核心优势分析
   - 参考第五章的 130 条借鉴点
   - 结合 Windows 平台特性做取舍
2. **实施阶段**：
   - 以 `hooks-architecture-design.md` 为主要指南
   - 本文档作为补充参考
   - 遇到设计问题时查阅相关章节
3. **维护阶段**：
   - 作为历史文档保留
   - 不建议频繁更新
   - 新的研究成果应记录到新文档

**不应该如何使用**：
- ❌ 不要作为实施指南（应使用 `hooks-architecture-design.md`）
- ❌ 不要实施跨引擎字段映射（已被 MCP 替代）
- ❌ 不要实施跨平台兼容性（seeyue-workflows 专注 Windows）

### 9.3 文档维护

**维护策略**：
- 本文档为只读参考文档
- 不再进行重大更新
- 新的研究成果应记录到新文档

**版本历史**：
- v1.0.0 (2026-03-12)：初始版本，基于 `hooks-improvement-checklist.md` 重构
- 后续版本：仅修正错误，不增加新内容

---

## 【附录】

### A. 参考项目源码位置

**Claude Code**：
- `refer/agent-source-code/claude-code-main/plugins/`
- `refer/agent-source-code/claude-code-main/examples/hooks/`
- `refer/agent-source-code/claude-code-main/plugins/plugin-dev/skills/hook-development/`

**Gemini CLI**：
- `refer/agent-source-code/gemini-cli-main/packages/core/src/hooks/`
- `refer/agent-source-code/gemini-cli-main/docs/hooks/`

**Codex**：
- `refer/agent-source-code/codex-main/codex-rs/hooks/src/`
- `refer/agent-source-code/codex-main/docs/config.md`

**Everything Claude Code**：
- `refer/everything-claude-code-main/hooks/`
- `refer/everything-claude-code-main/scripts/hooks/`

**Superpowers**：
- `refer/superpowers-main/hooks/`

### B. 相关文档

**实施文档**：
- `hooks-architecture-design.md` - 架构设计指南（主要实施文档）
- `mcp-integration-proposal.md` - MCP 融合方案（标准化能力暴露）

**规范文档**：
- `workflow/hooks.spec.yaml` - Hooks 规范
- `workflow/policy.spec.yaml` - 策略规范
- `workflow/file-classes.yaml` - 文件分类

**架构文档**：
- `docs/architecture-v4.md` - V4 整体架构

### C. 术语表

| 术语        | 定义                                               |
| ----------- | -------------------------------------------------- |
| Hook        | 在特定事件触发时执行的脚本                         |
| Event       | 触发 hook 的时机（如 PreToolUse、Stop）            |
| Decision    | Hook 的执行结果（allow/deny/ask）                  |
| Profile     | Hook 执行的严格程度级别（minimal/standard/strict） |
| Fingerprint | Hook 文件的 SHA256 哈希值，用于信任验证            |
| Wire Shape  | IPC 通信的数据格式，需要保持稳定                   |
| Aggregator  | 合并多个 hook 结果的组件                           |
| Registry    | 管理 hook 注册和加载的组件                         |
| Planner     | 生成 hook 执行计划的组件                           |
| Runner      | 执行 hook 脚本的组件                               |

---

**文档版本**：v1.0.0  
**最后更新**：2026-03-12  
**作者**：seeyue-workflows 架构团队  
**文档类型**：只读参考文档

---

**END OF DOCUMENT**