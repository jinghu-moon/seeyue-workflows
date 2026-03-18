# sy-interact TUI 架构设计

状态：draft  
阶段：P0  
适用范围：`sy-interact`、runtime、host wrapper、Hooks/MCP 交互层  
正式命名：本文统一使用 `sy-interact`，不再使用 `sy-interation` 这一拼写  

## 1. 目标

`sy-interact` 是 `seeyue-workflows` 的交互呈现器（presenter），负责把 runtime 生成的结构化交互请求渲染为终端内可操作的菜单界面，并把用户选择回写为结构化响应。

它的核心价值不是“再做一个 CLI 工具”，而是把以下能力从 prompt 文本层提升到正式交互层：

- 键盘优先的交互式菜单
- 风险显式的审批与恢复提示
- 多引擎共享的用户输入载体
- 与 runtime 状态一致的阻塞式 human-in-the-loop
- 结构化回写，而非自由文本协商

## 2. 问题背景

当前仓库已经具备：

- 以 `workflow/*.yaml` 为真相源的 control plane
- `scripts/runtime/*` 为中心的状态机与恢复逻辑
- `scripts/hooks/*` 与 `hook-client.cjs` 的边界拦截能力
- `seeyue-mcp` 提供 resources / prompts / tools 的 MCP 能力

但在“如何向用户发问并稳定收回答案”这一层，仍缺少统一组件。

如果继续用纯文本问答，会出现：

- 每个引擎的审批、恢复、问题澄清体验不一致
- 菜单和说明文本无法结构化回写
- 高风险交互难以做到键盘优先和颜色辅助
- 交互文案和运行时状态容易逐步分叉

因此需要一个独立的交互呈现器。

## 3. 核心判断

### 3.1 `sy-interact` 是 presenter，不是 policy engine

`sy-interact` 只负责：

- 读取交互请求
- 探测终端能力
- 渲染界面
- 处理键盘事件
- 回写用户响应

`sy-interact` 不负责：

- 决定是否需要审批
- 计算 `recommended_next`
- 解释 `restore_reason`
- 评估 Hooks / policy / runtime 的业务规则

这些都必须由 runtime / kernel 负责。

### 3.2 `sy-interact` 不应直接嵌入 `seeyue-mcp` 主循环

当前 `seeyue-mcp` 走的是 stdio MCP server，`stdout` 需要保持 JSON-RPC 干净；已有终端渲染逻辑也明确要求彩色输出走 `stderr`。因此，`sy-interact` 不应作为 `seeyue-mcp` 主进程的一部分直接管理 raw mode、alternate screen 和键盘事件，而应作为独立 presenter 二进制存在。

### 3.3 `sy-interact` 应由 host 驱动，而不是由 agent 临时决定

最佳流程不是让 agent 在任意时刻自己执行一条 shell 命令拉起 TUI，而是：

1. runtime 生成交互请求
2. host / wrapper 检测到 pending interaction
3. host 拉起 `sy-interact`
4. 用户完成选择
5. `sy-interact` 回写响应
6. runtime 恢复执行

这样才能避免把交互体验耦合到具体模型当轮的 prompt 决策里。

## 4. Component Map

| Component | Responsibility | Interface | Constraints |
|---|---|---|---|
| Runtime Kernel | 生成 `interaction_request`，消费 `interaction_response` | `.ai/workflow/interactions/*.json` | 唯一决策中心 |
| Host Wrapper | 监听 pending interaction 并拉起 `sy-interact` | process spawn / file watch | 不解释业务语义 |
| `sy-interact` | 渲染 TUI，处理键盘，回写响应 | request file / response file | 不承载策略判断 |
| Hook Client | 把 hook blocker 翻译为交互请求 | decision envelope | 不直接打开 TUI |
| MCP Server | 暴露交互状态资源、订阅、elicitation 能力 | resources / prompts / tools | 不替代本地 presenter |
| Durable Store | 保存请求、响应、事件、快照 | `.ai/workflow/interactions/` | 可恢复、可审计 |

## 5. Tech Stack Decisions

| Area | Chosen | Alternative | Reason |
|---|---|---|---|
| TUI 框架 | `ratatui + crossterm` | `dialoguer` / Node prompts | 支持布局、颜色、键盘事件、详情区、输入框 |
| 进程模型 | 独立二进制 | 内嵌到 `seeyue-mcp` | 避免污染 MCP stdio 和生命周期耦合 |
| 请求协议 | request/response JSON 文件 | 长参数 CLI | 更可扩展、可审计、可重放 |
| 颜色能力 | `mono`/`ansi16`/`ansi256`/`rgb24` 分级 | 默认强依赖真彩 | 适配 Windows Terminal、VS Code、普通终端 |
| 交互驱动 | host wrapper 拉起 | agent 自主 shell 拉起 | 更稳、更符合 human-in-the-loop |
| 结果模型 | 结构化 response + 可选 comment | 纯文本 | 便于 runtime 回写与冲突检测 |

## 6. Data Model

`sy-interact` 不直接定义业务真相源，但必须消费并产出以下对象：

- `interaction_request`
- `interaction_option`
- `interaction_presentation`
- `interaction_response`
- `interaction_response_analysis`

这组对象的正式机器定义放在：`workflow/interaction.schema.yaml`

### 6.1 最小请求对象

请求对象至少应包含：

- `interaction_id`
- `kind`
- `title`
- `message`
- `risk_level`
- `options`
- `presentation`
- `comment_mode`
- `originating_request_id`

### 6.2 最小响应对象

响应对象至少应包含：

- `interaction_id`
- `status`
- `answer`
- `selected_option_ids`
- `comment`
- `submitted_at`
- `presenter`

## 7. Integration Points

### 7.1 Runtime → `sy-interact`

runtime 负责生成交互请求文件，并把 session 置于可阻塞状态。`sy-interact` 只读取请求，不自行推断业务意图。

### 7.2 `sy-interact` → Runtime

`sy-interact` 写入响应文件后，由 host 或 runtime 消费结果并触发下一次状态推进。

### 7.3 Hooks → `sy-interact`

Hooks 不直接调用 `sy-interact`。正确路径是：Hook → Hook Client → Runtime → Interaction Request → Host → `sy-interact`。

### 7.4 MCP → `sy-interact`

MCP 提供远程/结构化交互总线，例如：

- resources 暴露 pending interactions
- `subscribe` / `listChanged` 提示交互状态变化
- `elicitation` 在支持的客户端内直接收集输入

当客户端不支持内建交互能力时，再由本地 host 拉起 `sy-interact`。

## 8. 用户交互模型

### 8.1 设计原则

- 菜单优先
- 键盘优先
- 颜色辅助
- 文本补充兜底
- 主答案结构化
- 冲突必须澄清

### 8.2 标准布局

推荐布局固定为六区：

1. 标题区
2. 原因/风险区
3. 影响范围摘要区
4. 主菜单区
5. 补充说明输入区
6. 快捷键提示区

### 8.3 键位约定

| Key | Meaning |
|---|---|
| `↑` / `k` | Move focus up |
| `↓` / `j` | Move focus down |
| `Enter` | Submit / confirm |
| `Space` | Toggle selection in multi-select |
| `Tab` | Focus comment input |
| `Shift+Tab` | Return to options |
| `Esc` | Cancel / go back |
| `?` | Help |
| `d` | Toggle details |

### 8.4 菜单类型

V1 建议支持：

- `approval_request`
- `restore_request`
- `question_request`
- `input_request`
- `conflict_resolution`
- `handoff_notice`

回答形式至少支持：

- `boolean`
- `single_select`
- `multi_select`
- `text`
- `number`
- `path`
- `secret`

### 8.5 固定补充说明框

每个交互默认保留一个位于底部的“补充说明（可选）”输入框，其语义是补充约束，而不是主答案。

规则：

- 结构化答案优先
- comment 默认只补充，不覆盖主答案
- comment 与主答案冲突时必须重新澄清
- `secret` 不应走普通 comment 通道

## 9. 终端能力模型

### 9.1 颜色等级

`sy-interact` 必须支持以下显示等级：

- `mono`
- `ansi16`
- `ansi256`
- `rgb24`

### 9.2 颜色 token

UI 不直接依赖具体 RGB 值，而是依赖语义 token：

- `danger`
- `warning`
- `success`
- `info`
- `focus`
- `muted`

再根据终端能力映射为具体颜色。

### 9.3 终端检测

建议优先检测：

- 是否 TTY
- 是否支持 ANSI VT
- 是否处于 Windows Terminal / VS Code integrated terminal
- 用户是否显式指定 `--color=never`

## 10. 运行模式

### 10.1 首选模式：TUI

当检测到 TTY 且能力满足要求时，进入：

- raw mode
- alternate screen
- 全屏或半屏菜单渲染

### 10.2 降级模式：Text Menu

当环境支持 ANSI 但不适合全屏 TUI 时，降级为：

- 编号菜单
- 单行高亮
- 普通输入提示

### 10.3 最低模式：Plain Prompt

当环境无 TTY 或颜色不可用时，降级为：

- 纯文本问题
- 稳定选项 ID
- 结构化 response 仍保持不变

## 11. Non-Goals

- 不在 `sy-interact` 中实现 policy engine
- 不在 `sy-interact` 中解析 `workflow/*.yaml` 全量规则
- 不让 `sy-interact` 直接承担 MCP server 职责
- 不把 comment 输入框升级为自由聊天窗口
- 不在 P0 阶段引入 GUI

## 12. Top Risks

1. **高风险：把 presenter 做成策略引擎**  
   缓解：严格把业务判断留在 runtime / kernel。

2. **高风险：把 TUI 嵌入 MCP stdio 主进程**  
   缓解：独立二进制，明确 stdout/stderr 边界。

3. **中风险：不同终端的颜色和按键行为不一致**  
   缓解：颜色分级、键位回退、text menu fallback。

4. **中风险：comment 与主答案冲突导致解释歧义**  
   缓解：引入冲突检测与二次澄清。

## 13. 参考依据

### 项目与 refer

- `docs/hooks-mcp-interaction-refactor-plan.md`
- `docs/architecture-v4.md`
- `refer/skills-and-hooks-architecture-advisory.md`
- `refer/agent-source-code/gemini-cli-main/docs/hooks/reference.md`
- `refer/agent-source-code/gemini-cli-main/docs/cli/checkpointing.md`
- `refer/agent-source-code/codex-main/docs/agents_md.md`

### 官方资料

- Windows Console VT Sequences  
  https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences
- VS Code Terminal Appearance  
  https://code.visualstudio.com/docs/terminal/appearance
- Anthropic Claude Code Hooks Reference  
  https://docs.anthropic.com/en/docs/claude-code/hooks
- MCP Resources / Prompts / Lifecycle  
  https://modelcontextprotocol.io/docs/concepts/resources  
  https://modelcontextprotocol.io/specification/2024-11-05/server/prompts  
  https://modelcontextprotocol.io/specification/2024-11-05/basic/lifecycle
- Ratatui / Crossterm 官方文档  
  https://docs.rs/ratatui/latest/ratatui/  
  https://docs.rs/crossterm/latest/crossterm/style/enum.Color.html
