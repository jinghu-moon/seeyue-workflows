# sy-interact CLI 契约规范

状态：draft  
阶段：P0  
适用范围：host wrapper、runtime、测试夹具、未来 dashboard 集成  
正式命名：`sy-interact`

## 1. 目标

本文定义 `sy-interact` 的 CLI 入口、输入输出契约、退出码与降级策略，确保它可以被 runtime、host wrapper、测试脚本和未来的 UI 外壳以一致方式调用。

## 2. 设计原则

- file-first，而不是 arg-first
- presenter-only，而不是 policy engine
- keyboard-first，但允许非 TTY 降级
- response 必须结构化
- 退出码表达进程层语义，业务语义放到 response

## 3. 命令面

P0 建议只提供两个子命令：

- `render`
- `probe-terminal`

### 3.1 `render`

用途：读取交互请求文件，渲染交互菜单，并写出响应文件。

建议签名：

```text
sy-interact render \
  --request-file <path> \
  --response-file <path> \
  [--color auto|always|never] \
  [--color-depth auto|16|256|24] \
  [--theme auto|dark|light] \
  [--mode auto|tui|text|plain] \
  [--timeout-seconds <int>] \
  [--no-alternate-screen]
```

### 3.2 `probe-terminal`

用途：探测当前终端能力，用于 host 启动前决策与测试。

建议签名：

```text
sy-interact probe-terminal [--format json|text]
```

输出至少包含：

- `is_tty`
- `ansi_enabled`
- `color_depth`
- `supports_raw_mode`
- `supports_alternate_screen`
- `preferred_mode`

## 4. 输入协议

### 4.1 请求文件

`render` MUST 接受一个 `interaction_request` JSON 文件路径。该文件格式由 `workflow/interaction.schema.yaml` 定义。

最小示例：

```json
{
  "schema": 1,
  "interaction_id": "ix-20260318-001",
  "kind": "approval_request",
  "status": "pending",
  "title": "高风险写入确认",
  "message": "即将覆盖 workflow/*.yaml",
  "risk_level": "high",
  "selection_mode": "single_select",
  "options": [
    { "id": "approve_once", "label": "确认继续", "recommended": true },
    { "id": "deny", "label": "拒绝" },
    { "id": "show_details", "label": "查看详情" }
  ],
  "comment_mode": "optional",
  "presentation": {
    "mode": "tui_menu",
    "color_profile": "auto",
    "theme": "auto"
  },
  "originating_request_id": "req-123"
}
```

### 4.2 非目标输入

P0 不建议支持以下输入方式作为主路径：

- 通过大量 `--title` / `--option` 参数直接拼装请求
- 通过环境变量承载完整业务请求
- 通过 stdin 临时喂入非结构化文本

这些方式可留作调试或测试夹具用途，但不应成为主调用路径。

## 5. 输出协议

### 5.1 响应文件

`render` MUST 产出一个 `interaction_response` JSON 文件。

最小示例：

```json
{
  "schema": 1,
  "interaction_id": "ix-20260318-001",
  "status": "answered",
  "answer_form": "single_select",
  "selected_option_ids": ["approve_once"],
  "comment": "仅允许本次，且不要修改 docs 之外的文件",
  "submitted_at": "2026-03-18T13:55:00Z",
  "presenter": {
    "name": "sy-interact",
    "version": "0.1.0",
    "mode": "tui_menu",
    "color_depth": "rgb24"
  }
}
```

### 5.2 业务分析输出

P1 之后 MAY 追加 `response_analysis`，用于冲突检测与澄清信号，例如：

- `comment_conflicts_with_answer`
- `needs_clarification`
- `clarification_reason`

## 6. 退出码约定

退出码只表达 CLI/进程层结果，不表达业务授权结果。

| Exit Code | Meaning |
|---|---|
| `0` | Response written successfully |
| `1` | Internal error |
| `2` | User cancelled |
| `3` | Request validation failed |
| `4` | Timeout |
| `5` | Terminal unsupported and no fallback available |

说明：

- 用户选择“拒绝”时，仍应返回 `0`，因为这是有效业务响应，不是进程失败。
- 真正的业务语义由 `interaction_response.status` 与 `selected_option_ids` 表达。

## 7. 终端行为约定

### 7.1 stdout / stderr

- `stdout` SHOULD 保持机器可控，优先用于 `probe-terminal --format json`
- `stderr` MAY 用于调试日志或错误摘要
- `render` 主路径不应把交互 UI 混入 stdout 结构化输出

### 7.2 Raw Mode

当 `mode=tui` 时：

- MUST 在进入界面前启用 raw mode
- MUST 在退出时恢复终端状态
- MUST 处理 panic/异常路径下的恢复逻辑

### 7.3 Alternate Screen

P0 默认允许 alternate screen，但应支持 `--no-alternate-screen`，便于某些终端或调试环境禁用。

## 8. 调用流程建议

推荐由 host wrapper 采用以下流程：

1. 检测 `session.yaml` 或 interaction store 中是否存在 `pending` 请求
2. 调用 `sy-interact probe-terminal`
3. 根据能力选择 `tui` / `text` / `plain`
4. 调用 `sy-interact render`
5. 读取 response file
6. 触发 runtime 恢复执行

## 9. 测试夹具建议

为保证 CLI 契约稳定，建议增加以下夹具类型：

- `approval_request` happy path
- `multi_select` with comment
- `text fallback` when no TTY
- `cancelled` flow
- `timeout` flow
- `comment conflict` flow
- `probe-terminal` snapshot

## 10. Non-Goals

- 不在 P0 提供网络 transport
- 不在 P0 支持 GUI mode
- 不在 P0 提供复杂插件系统
- 不在 P0 支持无限制自定义 keymap

## 11. 参考依据

- `docs/interaction-tui-architecture.md`
- `workflow/interaction.schema.yaml`
- `seeyue-mcp/src/platform/terminal.rs`
- Microsoft VT Sequences  
  https://learn.microsoft.com/en-us/windows/console/console-virtual-terminal-sequences
- VS Code Terminal Appearance  
  https://code.visualstudio.com/docs/terminal/appearance
- Ratatui backend docs  
  https://docs.rs/ratatui/latest/ratatui/backend/

