# 跨引擎 Interaction 映射设计

状态：draft  
阶段：P1  
适用范围：Claude Code、Codex、Gemini CLI adapters、host wrapper、`sy-interact`

## 1. 目标

本文定义统一 interaction 语义如何映射到不同 agent 引擎，确保：

- 真相源统一
- 引擎体验尽量接近原生
- 不支持的能力走明确降级路径
- `sy-interact` 只补位，不破坏原生能力

## 2. 核心原则

- adapter 吸收差异，不创造第二份真相源
- 能用原生能力，就不要伪造 hooks
- `sy-interact` 是 presenter fallback，不是每个引擎都必须经过的一层
- runtime 始终输出统一的 interaction 对象

## 3. 引擎能力概览

| 能力 | Claude Code | Codex | Gemini CLI |
|---|---|---|---|
| 工具前拦截 | 强 | 弱（依赖 sandbox/approval） | 强 |
| 原生审批 | 强 | 中 | 强 |
| 原生 checkpoint/restore 语义 | 中 | 弱 | 强 |
| MCP 集成 | 中 | 强 | 强 |
| 交互式本地 UI | 弱 | 弱 | 弱 |
| 最适合 `sy-interact` 补位的点 | 审批/恢复 UI | 审批/恢复主 UI | 冲突/恢复/补充输入 UI |

## 4. Claude Code 映射

### 4.1 原生能力

Claude Code 的优势在于：

- 完整 hooks 生命周期
- `PreToolUse` / `PostToolUse` / `Stop` 等明确事件面
- permission request 语义清晰

### 4.2 建议映射

| 统一语义 | Claude 原生路径 | `sy-interact` 角色 |
|---|---|---|
| `approval_request` | permission / hook 输出 | 用于 richer local UI |
| `restore_request` | `Stop` / `PreCompact` 派生 | 本地恢复菜单 |
| `question_request` | prompt/skill + hook 辅助 | 需要更强本地菜单时兜底 |
| `input_request` | prompt/skill | 复杂输入 UI 兜底 |
| `conflict_resolution` | hook 返回 block + reason | 本地冲突菜单 |

### 4.3 设计建议

- Claude 保持 hook-first + permission-first
- 当需要更优雅的本地 human-in-the-loop 时，再由 host 拉起 `sy-interact`
- 不把 `sy-interact` 硬塞到每一个 hook 路径中

## 5. Codex 映射

### 5.1 原生能力

Codex 的边界更偏：

- `AGENTS.md` 分层上下文
- `approval_policy`
- `sandbox_mode`
- `mcp_servers`

它没有 Claude 式完整 `PreToolUse` 生命周期，因此不应强行模拟。

### 5.2 建议映射

| 统一语义 | Codex 原生路径 | `sy-interact` 角色 |
|---|---|---|
| `approval_request` | `approval_policy` + host | 主要本地审批 UI |
| `restore_request` | runtime + host | 主要本地恢复 UI |
| `question_request` | prompt / MCP / host | 本地问答菜单 |
| `input_request` | MCP / host | 路径、范围、参数输入 UI |
| `conflict_resolution` | runtime + host | 主要冲突菜单 |

### 5.3 设计建议

- Codex 采用 config-first，不做伪 hooks
- interaction 主要由 runtime + host 驱动
- `sy-interact` 在 Codex 下往往是主本地交互 UI，而不是补位层

## 6. Gemini CLI 映射

### 6.1 原生能力

Gemini CLI 的优势在于：

- `BeforeToolSelection`
- policy engine
- checkpointing
- trust 配置
- MCP 集成能力较强

### 6.2 建议映射

| 统一语义 | Gemini 原生路径 | `sy-interact` 角色 |
|---|---|---|
| `approval_request` | policy `ask_user` / trust | richer local UI fallback |
| `restore_request` | checkpoint + runtime | 本地恢复菜单 |
| `question_request` | MCP / prompt | 本地菜单兜底 |
| `input_request` | MCP elicitation | 本地输入 fallback |
| `conflict_resolution` | runtime + host | 本地冲突菜单 |

### 6.3 设计建议

- Gemini 尽量吃满 native policy / checkpoint / trust
- `sy-interact` 主要作为本地 richer presenter 和无原生输入能力时的补位

## 7. Primary / Fallback Matrix

| Scenario | Primary Path | Fallback Path |
|---|---|---|
| Local approval on Claude | native permission | `sy-interact` |
| Local approval on Codex | host + `sy-interact` | plain text prompt |
| Local approval on Gemini | native policy ask_user | `sy-interact` |
| Restore menu on any engine | host + `sy-interact` | text menu |
| Structured parameter input with MCP client | MCP elicitation | `sy-interact` |
| No TTY, no MCP interactive support | text resource/prompt | deferred manual handling |

## 8. Capability Gap 报告建议

adapter 层应输出一份 machine-readable capability-gap 报告，至少包含：

- engine name
- supported interaction kinds
- native approval support
- native restore/checkpoint support
- native prompt/input support
- fallback mode
- known downgrade notes

建议输出位置：

- `.ai/workflow/capability-gap.json`
- 或 adapter 产物目录中的 `capability-gap.<engine>.json`

## 9. 与 `sy-interact` 的边界

`sy-interact` 不是 adapter compiler，也不是引擎特化插件。

它的定位应始终是：

- 统一 presenter
- 本地 keyboard-first UI
- 可被任何 adapter 通过 host/wrapper 使用

因此：

- engine-specific policy 仍留在各 adapter / runtime
- `sy-interact` 只消费统一 interaction schema

## 10. Non-Goals

- 不为 Codex 发明 Claude 风格 hooks 生命周期
- 不把所有 Gemini 原生能力降级到本地 TUI
- 不把 Claude 的 permission model 替换为自定义 UI

## 11. Top Risks

1. **高风险：为了统一体验而抹平引擎差异**  
   缓解：始终坚持 native-first，fallback-second。

2. **中风险：把 `sy-interact` 误当成 adapter 一部分**  
   缓解：文档与代码都强调 presenter-only 边界。

3. **中风险：Codex 路径过度依赖 prompt 文本**  
   缓解：尽量通过 runtime + host + MCP 保持结构化交互。

## 12. 参考依据

### 项目与 refer

- `refer/skills-and-hooks-architecture-advisory.md`
- `refer/agent-source-code/codex-main/codex-rs/core/config.schema.json`
- `refer/agent-source-code/gemini-cli-main/docs/hooks/reference.md`
- `refer/agent-source-code/gemini-cli-main/docs/cli/checkpointing.md`
- `refer/agent-source-code/gemini-cli-main/docs/tools/mcp-server.md`

### 官方资料

- Claude Code Hooks  
  https://docs.anthropic.com/en/docs/claude-code/hooks
- MCP Overview / Lifecycle / Resources  
  https://modelcontextprotocol.io/docs/concepts/resources  
  https://modelcontextprotocol.io/specification/2024-11-05/basic/lifecycle
