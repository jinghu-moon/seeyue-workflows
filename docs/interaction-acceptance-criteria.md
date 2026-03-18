# Interaction 验收标准与测试矩阵

状态：draft  
阶段：P2  
适用范围：`sy-interact`、runtime、Hooks bridge、MCP integration、engine adapters

## 1. 目标

本文定义 interaction 体系的完成标准，用于判断：

- `sy-interact` 是否真正可用
- runtime / hooks / MCP 集成是否闭环
- 跨引擎体验是否达到“结构一致、能力降级可控”

## 2. 验收原则

- 先验证契约，再验证体验
- 先验证 blocker-first，再验证美观度
- 先验证降级路径，再验证真彩色/TUI 最优路径
- 先验证恢复和审批，再验证普通问答

## 3. P0 验收

### 3.1 文档与契约

必须存在并相互对齐：

- `docs/interaction-tui-architecture.md`
- `docs/sy-interact-cli-spec.md`
- `workflow/interaction.schema.yaml`

### 3.2 schema 验收

必须满足：

- request/response 都有稳定必填字段
- 支持 `comment_mode`
- 支持 `presentation.mode`
- 支持颜色能力分级
- 支持冲突分析字段

### 3.3 CLI 契约验收

必须满足：

- `render` 与 `probe-terminal` 两个子命令定义清晰
- 退出码只表达进程结果，不表达业务拒绝
- response 结构稳定可回写 runtime

## 4. P1 验收

### 4.1 Runtime 集成

必须满足：

- runtime 能创建 interaction request
- `session` 可投影 active interaction 状态
- `recommended_next` 可指向“先处理 interaction”
- interaction 完成后 runtime 可恢复推进

### 4.2 Host 集成

必须满足：

- host 能发现 pending interaction
- host 能根据终端能力选择 `tui` / `text` / `plain`
- host 能回收 response 并重新交给 runtime

### 4.3 Journal / checkpoint

必须满足：

- interaction 事件可写入 journal
- handoff / checkpoint 至少能保留 active interaction 摘要

## 5. P2 验收

### 5.1 Hooks bridge

必须满足：

- `hard_gate` hook failure 不 silently fail-open
- hook verdict 可稳定生成 interaction request
- hook 不直接拉起本地 UI
- post-tool telemetry failure 不生成多余 interaction

### 5.2 MCP bus

必须满足：

- 存在 active interaction 资源入口
- 支持订阅或变化通知设计
- tool 输出有 `structuredContent`
- 有 `elicitation` 与本地 presenter 的优先级规则

### 5.3 跨引擎映射

必须满足：

- Claude / Codex / Gemini 都有明确 primary/fallback path
- capability-gap 有输出方案
- 引擎差异主要体现在 adapter，不体现在 interaction schema 分叉

## 6. TUI 体验验收

### 6.1 键盘交互

必须满足：

- `↑/↓` 或 `j/k` 可移动焦点
- `Enter` 可确认
- `Space` 可切换多选
- `Tab` 可进入 comment 输入
- `Esc` 可取消或返回

### 6.2 颜色与布局

必须满足：

- 高风险项视觉上可一眼识别
- 当前焦点可明确高亮
- comment 区与主菜单区有清晰分区
- 在 `mono` 下仍然可理解，不依赖颜色单独传达语义

### 6.3 终端恢复

必须满足：

- raw mode 正常恢复
- panic / error 路径仍可恢复终端
- alternate screen 可配置关闭

## 7. 终端兼容矩阵

建议至少覆盖以下环境：

| Environment | Expected Mode |
|---|---|
| Windows Terminal | `tui` with `rgb24` preferred |
| VS Code Integrated Terminal | `tui` or `text`, true color preferred |
| PowerShell Console (modern) | `tui` or `text`, depending on VT support |
| Redirected / non-TTY shell | `plain` |

## 8. 场景测试矩阵

建议最少覆盖：

- 审批确认：单选 + comment
- 恢复菜单：单选 + details
- 多选范围授权：`Space` 勾选 + comment
- 路径输入：`path` form
- 冲突澄清：comment 与主答案冲突
- 用户取消：`Esc` / cancel path
- 超时：timeout path
- 无 TTY：plain fallback
- MCP 支持 `elicitation`：不拉起本地 TUI
- MCP 不支持 `elicitation`：回退到 `sy-interact`

## 9. 失败判定

满足以下任一情况，视为 interaction 体系尚未通过：

- presenter 结果无法稳定回写 runtime
- 高风险 blocker 仍可能绕过人工确认
- comment 与结构化答案冲突时系统静默吞掉冲突
- 无 TTY 环境下没有可用 fallback
- 终端退出后残留 raw mode / alternate screen
- Claude / Codex / Gemini 的交互对象不再共用一套 schema

## 10. 建议测试实现

建议后续实现以下测试层：

- schema validation tests
- CLI snapshot tests
- TUI interaction tests
- runtime integration fixtures
- hook bridge contract tests
- MCP resource / tool contract tests
- engine adapter conformance tests

## 11. 参考依据

- `docs/interaction-tui-architecture.md`
- `docs/sy-interact-cli-spec.md`
- `docs/interaction-runtime-integration.md`
- `docs/engine-interaction-mapping.md`
- `docs/hooks-interaction-bridge.md`
- `docs/mcp-interaction-bus.md`
- `workflow/interaction.schema.yaml`
- Windows VT Sequences  
  https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences
- VS Code Terminal Appearance  
  https://code.visualstudio.com/docs/terminal/appearance
- MCP Specification  
  https://modelcontextprotocol.io/specification/2024-11-05/
