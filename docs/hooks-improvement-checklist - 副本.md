# Hooks 改造建议清单（聚焦 Claude Code / Codex / Gemini CLI）

【一句话定性】
把 hooks 变成“跨引擎一致的执行边界”，让同一条规则在 Claude Code / Codex / Gemini CLI 中都得到同样的约束和证据。

【核心逻辑】
为什么要做：seeyue-workflows 负责跨引擎协作，如果 hooks 行为不一致，就会出现“在 A 能拦、在 B 放过”的漏洞。  
第一步：统一 Hook 事件矩阵与 IPC 合约，适配器只做字段翻译。  
第二步：Hook Client 读取运行态并调用 Policy Kernel，输出标准化决策。  
第三步：日志、审计、输出模板统一落盘，支持开关与回归验证。

【重点借鉴点（按引擎）】
1. Claude Code：PreToolUse 可改写输入、Stop 可阻断、PermissionRequest 可自动审批；hook 可绑定到 skill/agent/frontmatter；支持 `once`、`timeout`、HTTP hooks。  
2. Codex：hook 只覆盖 AfterAgent / AfterToolUse，不可前置阻断；payload 提供 tool kind、sandbox policy、output preview，适合审计与通知。  
3. Gemini CLI：事件覆盖工具/代理/模型三层；支持 sequential/parallel、toolConfig 过滤与 union 合并；配置分层、指纹信任、CLI 管理 hooks。  
4. 补充：Superpowers/ECC 在跨平台稳定性、profile 开关、hooks 测试与提示型规则上有成熟经验。

【补充借鉴点（关键细节）】
1. Claude Code：Stop/SubagentStop 输入包含 `last_assistant_message`，不必读 transcript；PreToolUse 可返回 `additionalContext`；PermissionRequest 允许 ask/allow/deny 与输入改写同场景共存。  
2. Claude Code：官方说明 `sandbox` 只作用于 Bash，不覆盖 hooks；managed settings 可禁用用户或项目 hooks。  
3. Claude Code plugin-dev：提供 hook-linter、validate-hook-schema、test-hook；推荐“快检命令 + 深检 prompt”双层校验。  
4. Codex：HookResult 分 Success/FailedContinue/FailedAbort 并短路；HookPayload 有稳定 JSON wire shape 测试。  
5. Gemini CLI：BeforeModel 可直接替换请求或注入合成响应；AfterTool 可隐藏真实输出并替换 reason；BeforeAgent 可丢弃用户输入。  
6. Gemini CLI：hooksConfig.enabled/disabled list/通知开关齐全，配置按 project/user/system/extension 合并；提供 CLI 管理命令与环境变量别名兼容。  
7. Claude Code：hook 可为 prompt 或 command；复杂规则用 prompt、确定性规则用 command，避免把“可解释判断”写进 bash。  
8. Claude Code：并行 hooks 不保证顺序，要求“互不依赖”；需要顺序时必须显式串行化。  
9. Claude Code：可用 flag file 方式做“临时启用/禁用”或“只在 CI 执行”，减少日常噪音。  
10. Codex：HookToolInput 明确区分 Function/Custom/LocalShell/MCP；LocalShell 包含 sandbox 权限、prefix_rule 与 justification，适合做细粒度审计。  
11. Codex：通知 hook 采用 argv + JSON 末参的 fire-and-forget 模式，不应依赖 stdout/stderr。  
12. Gemini CLI：stdout 污染会被当作 systemMessage 并默认放行，强调“严格 JSON”与 fail-open。  
13. Gemini CLI：项目 hooks 会被指纹校验，命令或名称变更会被视为不受信任。  
14. Gemini CLI：hook name 会进入 telemetry，默认脱敏（可选开启完整记录），避免泄露敏感参数。  
15. Claude Code：hook 输入包含 `hook_event_name`、`agent_id`、`agent_type` 等元数据，适合多代理审计与路由。  
16. Claude Code：PostToolUse 输出有“收敛展示”策略，可借鉴为 output.log 的“短摘要 + 证据指针”。  
17. Claude Code：`once: true` 限制单次运行，适合启动提示与一次性初始化。  
18. Codex：AfterToolUse payload 提供 `mutating`、`duration_ms`、`sandbox`、`sandbox_policy`、`output_preview`，适合统一审计字段。  
19. Codex：HookResult 的 abort 会短路后续 hooks，适合“必须阻断”的硬门。  
20. Gemini CLI：hooks 组级 `sequential` 控制串行，默认并行，适合竞争资源或有依赖的场景。  
21. Gemini CLI：`suppressOutput` 可隐藏 hook 元数据，适合敏感环境。  
22. Gemini CLI：CLI 提供 `/hooks enable-all|disable-all|enable <name>|disable <name>`，可作为 seeyue 的交互参考。  
23. Gemini CLI：hook 环境经过 sanitization，建议 seeyue 也最小化 env 透传并显式允许。  
24. Gemini CLI：toolConfig 支持 `mode: NONE` 强制禁用；多 hook whitelist 做 union 合并。  
25. Claude Code：HTTP hooks 允许远程策略服务，适合企业安全集成，但需 fail-open。  
26. Claude Code：`PermissionRequest` hook 可处理 always allow 建议并更新权限，适合自动化审批流。  
27. Claude Code：hooks timeout 上限较高，需设置合理上限并避免阻塞核心路径。  
28. Claude Code：`CLAUDE_CODE_SIMPLE` 禁用 hooks，需要在 seeyue 中显式检测并降级为纯审计。  
29. Claude Code：hooks 在会话启动时加载，运行中变更不生效，测试必须重启会话。  
30. Claude Code：插件 hooks 与用户 hooks 会合并并并行执行，需要明确优先级与冲突策略。  
31. Gemini CLI：基础输入包含 `transcript_path` 与 `timestamp`，适合做统一审计定位。  
32. Gemini CLI：BeforeToolSelection 不支持 `decision`/`continue`/`systemMessage`，只接受 toolConfig。  
33. Codex：HookPayload 固定 `event_type` + `event` 结构，并用序列化测试固化 wire shape，减少兼容破坏。  
34. Gemini CLI：exit code 0=成功解析 JSON，2=阻断，其他=警告继续，需要统一错误分级语义。  
35. Claude Code：输入含 `permission_mode`、`tool_use_id`、`agent_type`、`agent_transcript_path` 等字段，适合打通工具链追踪。  
36. Gemini CLI：hooks 同步阻塞，CLI 等待所有匹配 hooks 完成；重型逻辑必须缓存并收窄 matcher。  
37. Gemini CLI：exit code 2 在不同事件有不同行为，AfterAgent 触发重试、Tool 仅阻断该工具。  
38. Gemini CLI：配置层级为 project > user > system > extension，`/hooks panel` 可看执行计数与失败原因。  
39. Gemini CLI：提供环境变量脱敏与 allowlist（默认关闭），适合对第三方 hooks 降风险。  
40. Codex：hooks 顺序执行且遇 abort 立刻短路；legacy notify 走 argv + JSON 末参并 stdio 置空。  
41. ECC：hooks 支持 `async` 与 `timeout`，配合 `run-with-flags` 做 profile gating（minimal/standard/strict）与禁用列表。  
42. ECC：`run-with-flags` 限制 stdin 大小并在禁用/缺失脚本时原样透传，降低失败面。  
43. ECC：SessionStart 通过多路径搜索 plugin root，避免 `CLAUDE_PLUGIN_ROOT` 缺失导致初始化失败。  
44. Superpowers：用 polyglot `run-hook.cmd` 解决 Windows bash 缺失与 .sh 自动检测问题，缺 bash 时静默放行。  
45. Hookify：规则文件热更新、warn/block 分级、多规则聚合，并支持从 `transcript_path` 读取上下文。  
46. Claude Code：13 事件生命周期含 `PermissionRequest` / `PostToolUseFailure` / `SubagentStart` / `SubagentStop` / `PreCompact` / `SessionEnd`，事件矩阵需标注“可阻断/仅观察”。  
47. Advisory：Hook IPC 必须版本化 Input/Output Envelope；Hook 脚本只做 stdin/stdout 转换，运行态读取统一走 Hook Client。  
48. Patch：`journal.jsonl` 必须 O_APPEND 追加，禁止“读-改-写”；单行 <4KB；Windows 用串行队列或锁。  
49. V3 语义：`PostToolUse` 不得阻断主流程；昂贵检查异步化并写机器日志；`stderr` 无 exit 2 仅警告。  
50. V3 Stop：使用文件锁 + 过期时间避免 Stop hook 重入循环。  
51. Gemini：检查点必须是“变更前快照”，在 mutating tool 之前创建。  
52. Gemini：policy engine 负责 allow/ask/deny，hooks 只做证据与状态同步。  
53. V4：统一 Approval Envelope（action/target/risk/scope），避免各引擎审批字段漂移。  
54. Phase5：hook 的阻断/错误输出用模板渲染（error-report），统一文案与结构。  
55. Stop 语义：`decision=block` 的含义是“继续执行”，exit 2 仅作为错误中断，不用于继续。  

【字段级映射表（跨引擎最小一致）】
为什么：字段名不对齐会让适配器逻辑分散，难以做统一回归。  
怎么做：先定义 seeyue 最小字段集，再为缺失字段给出明确降级策略。  

【输入包络字段映射（建议）】
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `event_name` | `hook_event_name` | `hook_event.event_type` | `hook_event_name` | Codex 从 `event_type` 提取 |
| `session_id` | `session_id` | `session_id` | `session_id` | 一致 |
| `cwd` | `cwd` | `cwd` | `cwd` | 一致 |
| `transcript_path` | `transcript_path` | 无 | `transcript_path` | Codex 侧由 seeyue 生成并落盘 |
| `timestamp` | 无 | `triggered_at` | `timestamp` | 缺失时由 seeyue 补齐 |
| `engine` | 无 | `client`(可选) | 无 | 由适配器注入 |

【工具事件字段映射（建议）】
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `tool_name` | `tool_name` | `tool_name` | `tool_name` | 一致 |
| `tool_kind` | 无 | `tool_kind` | 由 `tool_name` 推断 | 无则空值 |
| `tool_input` | `tool_input` | `tool_input`(含 `input_type`) | `tool_input` | Codex 保留 `input_type` |
| `tool_result` | `tool_result` | `output_preview` + `success` | `tool_response` | Codex 仅预览，标注为 partial |
| `mcp_context` | 无 | `tool_kind=mcp` + `tool_input` | `mcp_context` | Claude Code 侧为空 |

【调用链路字段映射（建议）】
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `tool_call_id` | `tool_use_id` | `call_id` | 无 | 缺失时由 seeyue 生成 |
| `turn_id` | 无 | `turn_id` | 无 | 由 seeyue 生成 |
| `thread_id` | 无 | `thread_id` | 无 | 用 `session_id` 代替 |
| `original_request_name` | 无 | 无 | `original_request_name` | 仅 Gemini 有 |
| `agent_id` | `agent_id` | 无 | 无 | 为空 |
| `agent_type` | `agent_type` | 无 | 无 | 为空 |
| `agent_transcript_path` | `agent_transcript_path` | 无 | 无 | 为空 |
| `stop_hook_active` | 无 | 无 | `stop_hook_active` | 仅 Gemini 有 |

【执行结果字段映射（建议）】
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `tool_executed` | 无 | `executed` | 无 | seeyue 内部推断 |
| `tool_success` | 无 | `success` | `tool_response.error` 为空 | 缺失时标记 unknown |
| `duration_ms` | 无 | `duration_ms` | 无 | seeyue 内部计时 |
| `mutating` | 无 | `mutating` | 无 | seeyue 内部判定 |
| `sandbox` | 无 | `sandbox` | 无 | seeyue 内部注入 |
| `sandbox_policy` | 无 | `sandbox_policy` | 无 | seeyue 内部注入 |
| `output_preview` | `tool_result` 截断 | `output_preview` | `tool_response.returnDisplay` | 统一裁剪长度 |

【权限与安全字段映射（建议）】
注：本节包含 hook 输入与配置级字段，主要用于统一策略与审计。  
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `permission_mode` | `permission_mode` | 无 | 无 | 为空 |
| `sandbox_permissions` | 无 | `tool_input.local_shell.sandbox_permissions` | 无 | 为空 |
| `env_redaction_enabled` | 无 | 无 | `security.environmentVariableRedaction.enabled` | 仅 Gemini 有 |

【代理/停止事件字段映射（建议）】
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `user_prompt` | `user_prompt` | `input_messages` | `prompt` | Codex 做拼接并标注来源 |
| `assistant_response` | `last_assistant_message` | `last_assistant_message` | `prompt_response` | 统一映射为响应文本 |
| `stop_reason` | `reason` | 无 | `stopReason` | 缺失时落盘为空 |

【输出决策字段映射（建议）】
| seeyue 决策 | Claude Code | Codex | Gemini CLI | 说明 |
| --- | --- | --- | --- | --- |
| `allow` | `permissionDecision=allow` | `HookResult::Success` | `decision=allow` | 默认放行 |
| `deny` | `permissionDecision=deny` 或 exit 2 | `HookResult::FailedAbort` | `decision=deny` 或 exit 2 | 硬阻断 |
| `ask` | `permissionDecision=ask` | 无 | 无 | 仅 Claude Code 有原生语义 |

【输出控制字段映射（建议）】
| seeyue 字段 | Claude Code | Codex | Gemini CLI | 降级策略 |
| --- | --- | --- | --- | --- |
| `system_message` | `systemMessage` | 无 | `systemMessage` | Codex 仅日志 |
| `continue` | `continue` | 无 | `continue` | Codex 仅日志 |
| `stop_reason` | `stopReason` 或 `reason` | 无 | `stopReason` | Claude 缺失时用 `reason` |
| `decision` | `decision`(Stop) / `permissionDecision`(PreToolUse) | `HookResult` | `decision` | Codex 仅 abort/continue |
| `reason` | `reason` | 错误文本 | `reason` | 缺失时空 |
| `additional_context` | `hookSpecificOutput.additionalContext` | 无 | `hookSpecificOutput.additionalContext` | 不支持则忽略 |
| `input_mutation` | `hookSpecificOutput.updatedInput` | 无 | `hookSpecificOutput.tool_input` | 不支持则忽略 |
| `permission_decision_reason` | `hookSpecificOutput.permissionDecisionReason` | 无 | 无 | 仅 Claude |
| `tool_config` | 无 | 无 | `hookSpecificOutput.toolConfig` | 仅 Gemini |
| `llm_response` | 无 | 无 | `hookSpecificOutput.llm_response` | 仅 Gemini |
| `clear_context` | 无 | 无 | `hookSpecificOutput.clearContext` | 仅 Gemini |

【跨引擎最小可用 Hook 模板】
为什么：最小模板能快速验证“输入 → 决策 → 回写”链路是否对齐。  
第一步：用最小输入触发事件。  
第二步：只返回必要字段，避免 stdout 污染。  
第三步：把模板固化为 fixture 做回归。  
落点：`tests/hooks/fixtures/*.json`、`tests/hooks/run-hook-template-fixtures.cjs`。  

Claude Code（PreToolUse，命令 hook）
输入（stdin）:
```json
{
  "hook_event_name": "PreToolUse",
  "session_id": "s-1",
  "cwd": "/repo",
  "transcript_path": "/tmp/claude.json",
  "tool_name": "Write",
  "tool_input": { "file_path": "/repo/a.txt", "content": "hello" }
}
```
输出（stdout，允许）:
```json
{
  "hookSpecificOutput": { "permissionDecision": "allow" }
}
```
输出（stdout，阻断）:
```json
{
  "hookSpecificOutput": { "permissionDecision": "deny" },
  "systemMessage": "Blocked by policy"
}
```

Gemini CLI（BeforeTool，命令 hook）
输入（stdin）:
```json
{
  "hook_event_name": "BeforeTool",
  "session_id": "s-1",
  "cwd": "/repo",
  "transcript_path": "/tmp/gemini.json",
  "timestamp": "2025-01-01T00:00:00Z",
  "tool_name": "write_file",
  "tool_input": { "path": "/repo/a.txt", "content": "hello" }
}
```
输出（stdout，允许）:
```json
{
  "decision": "allow"
}
```
输出（stdout，阻断）:
```json
{
  "decision": "deny",
  "reason": "Blocked by policy"
}
```

Codex（AfterToolUse，Rust HookFn）
```rust
async fn audit_hook(payload: &HookPayload) -> HookResult {
    let event = &payload.hook_event;
    match event {
        HookEvent::AfterToolUse { event } => {
            if event.mutating && event.sandbox_policy == "danger-full-access" {
                return HookResult::FailedAbort("unsafe".into());
            }
        }
        _ => {}
    }
    HookResult::Success
}
```

【改造清单（核心能力，必须对齐三大引擎）】
1. 统一 Hook 事件矩阵与映射  
为什么：事件集不一致会造成规则缺失或重复触发。  
怎么做：定义 seeyue 事件矩阵，并给 Claude Code / Gemini / Codex 做明确映射；缺失事件要有降级策略。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/hooks/gemini-hook-bridge.cjs`。  
验证：为每个引擎输出“事件 → 触发结果”快照 fixture。  
风险：事件过多会变复杂，先覆盖现有 sy-hook 的核心事件。

2. IPC 合约与“严格 JSON”规则  
为什么：stdout 混入非 JSON 会让引擎解析失败，导致误放行。  
怎么做：stdout 只输出 JSON；stderr 仅日志；解析失败时 fail-open，并记录警告。  
落点：`scripts/hooks/sy-hook-lib.cjs`。  
验证：新增 stdout 污染用例，确保仍可继续执行。  
风险：历史脚本需适配，保留兼容期。

3. Hook 作用域最小化（按 skill/agent）  
为什么：全局 hooks 容易误伤无关任务。  
怎么做：编译时将 hooks 绑定到具体 skill/agent 生命周期；保留全局兜底安全门。  
落点：`scripts/runtime/skills-manifest.cjs`、`scripts/runtime/hook-client.cjs`。  
验证：仅在特定 skill 激活时触发对应 hooks。  
风险：遗漏兜底会导致安全空档。

4. PreToolUse 输入改写 + PermissionRequest 协同  
为什么：仅拦截不够，常常需要“修正参数 + 仍需审批”。  
怎么做：统一 `updatedInput` 与 `hookSpecificOutput.tool_input` 语义，审批流程允许改写后再 ask/allow/deny。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/runtime/approval-resolution.cjs`。  
验证：fixture 覆盖“改写输入 + 触发审批”。  
风险：改写必须写入审计日志，避免“隐形修改”。

5. 顺序控制（sequential/parallel）  
为什么：并行 hooks 会出现共享资源竞争，串行又会变慢。  
怎么做：按 hook 组提供 `sequential`，默认并行，仅共享资源使用串行。  
落点：`scripts/runtime/hook-client.cjs`。  
验证：并发竞争 fixture 与顺序 fixture。  
风险：串行过多会拖慢交互。

6. BeforeToolSelection 的 toolConfig 合并  
为什么：多个过滤器互相覆盖会导致误放行或误禁用。  
怎么做：对白名单做 union 合并；`mode: NONE` 作为强制禁用开关。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/hooks/sy-before-tool-selection.cjs`。  
验证：多 hook 合并 fixture。  
风险：合并逻辑复杂，必须有单测。

7. AfterTool 的 tailToolCallRequest 链式调用  
为什么：部分后置处理需要“立刻接一个工具调用”。  
怎么做：支持 tail tool request；引擎不支持时降级为 systemMessage 提示。  
落点：`scripts/runtime/hook-client.cjs`。  
验证：hook 返回 tail tool 时可执行并替换结果。  
风险：链式调用必须限制次数，防止无限循环。

8. AfterModel vs AfterAgent 的性能分层  
为什么：AfterModel 按流分片执行，成本极高。  
怎么做：仅将“实时脱敏/拦截”放在 AfterModel；其他校验放在 AfterAgent。  
落点：`scripts/hooks/sy-after-model.cjs`、`scripts/runtime/hook-client.cjs`。  
验证：同一规则在 AfterAgent 执行时不触发流式回放。  
风险：误用 AfterModel 会显著拖慢响应。

9. Hook timeout / once / 缓存策略  
为什么：慢 hooks 会冻结会话，重复执行浪费资源。  
怎么做：为每个 hook 设置 timeout；对重任务用 `once`；对昂贵检查做短期缓存。  
落点：`scripts/runtime/hook-client.cjs`。  
验证：超时 hook 能被正确终止并记录。  
风险：过度缓存会掩盖真实问题。

10. Hook 安全边界与信任机制  
为什么：Claude Code 的 sandbox 不保护 hooks，hooks 等同本地脚本权限。  
怎么做：hooks 标记为“高权限”；对项目级 hooks 做指纹校验，变更即提示。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/runtime/output-log.cjs`。  
验证：修改 hook 脚本后触发“未信任变更”提示。  
风险：提示太多会被忽略，需合并到单条警告。

11. Codex 兼容策略（只做审计/通知）  
为什么：Codex 只有 AfterAgent / AfterToolUse，无法前置阻断。  
怎么做：在 Codex 适配器中，将 hooks 定位为审计与通知；阻断靠 sandbox/approval。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/runtime/output-log.cjs`。  
验证：Codex 下 hooks 不产生阻断。  
风险：安全规则必须通过审批与 sandbox 落实。

12. Codex payload 的审计强化  
为什么：Codex 提供 `mutating`、`sandbox_policy`、`output_preview` 等高价值字段。  
怎么做：将这些字段写入审计与报告，作为危险操作证据。  
落点：`scripts/runtime/report-builder.cjs`、`scripts/runtime/output-log.cjs`。  
验证：报告中可看到 sandbox 与 mutating 标记。  
风险：output_preview 可能包含敏感内容，需脱敏。

13. Hook 日志脱敏与 suppressOutput  
为什么：hook 输入输出可能包含敏感参数。  
怎么做：默认脱敏 hook 名称与参数；支持 suppressOutput 抑制记录。  
落点：`scripts/runtime/output-log.cjs`。  
验证：带密钥参数的 hook 不会原文落盘。  
风险：过度脱敏会影响排障，保留 trace ID。

14. Hook 校验与测试体系  
为什么：hooks 失败会直接影响执行边界，必须可回归。  
怎么做：新增 hooks.json 校验脚本与 hook 集成测试，覆盖输入异常、超时、exit code。  
落点：`scripts/ci/validate-hooks.cjs`（新增）、`tests/hooks/`（新增）。  
验证：CI 强制通过 hooks 校验与核心用例。  
风险：测试过多会拖慢 CI，控制用例规模。

15. HTTP hooks（可选）  
为什么：企业策略服务需要远程判定。  
怎么做：Claude Code 走 HTTP hooks；其他引擎降级为本地脚本。  
落点：Claude Code adapter 编译输出。  
验证：HTTP hook 返回决策能被正确处理。  
风险：网络失败要 fail-open 并记录。  

16. Hook 结果分级与短路  
为什么：Codex 明确区分“失败但继续”和“失败即中止”，有利于稳定执行。  
怎么做：Hook Client 输出 `success / failed_continue / failed_abort`，调度器遇到 abort 立即短路。  
落点：`scripts/runtime/hook-client.cjs`。  
验证：fixture 覆盖三类结果与短路行为。  
风险：过度 abort 会降低可用性，需严格限制条件。  

17. Hook payload 稳定化与 schema 单测  
为什么：Codex 为 payload 提供稳定 wire shape 测试，防止破坏兼容性。  
怎么做：新增 hook payload schema 与序列化快照测试，确保字段与命名不漂移。  
落点：`scripts/runtime/spec-validator.cjs`、`tests/hooks/`（新增）。  
验证：schema 变更必须同时更新测试快照。  
风险：测试过严会降低迭代速度，需配套变更流程。  

18. 退出码与 decision 统一策略  
为什么：Gemini CLI 把 exit 0 作为“结构化决策”，exit 2 作为“紧急阻断”。  
怎么做：在 seeyue 内部统一为 `decision` + `reason` 输出，exit 2 仅保留硬阻断场景。  
落点：`scripts/hooks/sy-hook-lib.cjs`。  
验证：同一 hook 在不同引擎下语义一致。  
风险：exit 2 过多会导致无上下文阻断。  

19. BeforeAgent/AfterAgent 质量门与重试  
为什么：Gemini CLI 支持 BeforeAgent 拦截与 AfterAgent 自动重试；Claude Code 用 Stop 对齐。  
怎么做：建立“输入校验 → 输出复核/重试”的跨引擎映射；优先读取 `last_assistant_message`。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/hooks/sy-stop.cjs`。  
验证：触发错误输出后自动重试一次并记录原因。  
风险：重试必须有次数上限。  

20. AfterTool 输出隐藏与替换  
为什么：Gemini CLI 允许隐藏真实 tool 输出，适合敏感结果脱敏。  
怎么做：支持 `decision: deny` + `reason` 覆盖输出，Claude Code/Codex 无此能力时降级为 systemMessage。  
落点：`scripts/runtime/hook-client.cjs`。  
验证：敏感输出被替换，日志仅保留脱敏版本。  
风险：隐藏输出会降低调试效率，需保留审计证据。  

21. 快检 + 深检双层 hook  
为什么：Claude Code 推荐“命令快检 + prompt 深检”减少成本。  
怎么做：先跑轻量规则，命中风险再触发 prompt 深检；未命中直接放行。  
落点：`scripts/runtime/hook-client.cjs`、`scripts/hooks/sy-pretool-bash.cjs`。  
验证：安全命令只走快检，高风险命令走深检。  
风险：规则误判会触发不必要的 prompt。  

22. 运行模式与受管设置侦测  
为什么：Claude Code 允许 managed settings 禁用用户 hooks，`CLAUDE_CODE_SIMPLE` 会关闭 hooks。  
怎么做：启动时检测 hooks 是否被禁用，输出明确提示并降级为“仅审计”。  
落点：`scripts/hooks/sy-session-start.cjs`、`scripts/runtime/hook-client.cjs`。  
验证：禁用场景下不阻断且有提示。  
风险：过多提示会影响体验。  

【改造清单（体验增强，可选）】
1. SessionStart/Stop 摘要回灌  
为什么：长会话易断片，影响连续性。  
怎么做：Stop 写摘要块，SessionStart 注入 additionalContext。  
落点：`scripts/runtime/hook-client.cjs`。  
验证：新会话可自动读到上次摘要。  
风险：摘要过长要截断。

2. /compact 提醒与记录  
为什么：上下文膨胀时容易遗忘关键决策。  
怎么做：按工具调用数触发提示，并写入 output.log。  
落点：`scripts/hooks/sy-prompt-refresh.cjs`。  
验证：达到阈值后提示一次。  
风险：提示频率过高会干扰。

3. Hook profile 与禁用列表  
为什么：不同任务对严格度要求不同。  
怎么做：`SY_HOOK_PROFILE=minimal|standard|strict`，`SY_DISABLED_HOOKS=...`。  
落点：`scripts/hooks/sy-hook-lib.cjs`。  
验证：profile 切换后触发集合变化。  
风险：档位过多会混乱。

4. 文档路径提醒（非阻断）  
为什么：文档散落会难维护。  
怎么做：写入非 `docs/` 或非标准文件名时提示。  
落点：`scripts/hooks/sy-posttool-write.cjs`。  
验证：非标准路径触发提醒。  
风险：误报用“提醒不阻断”缓解。

5. console.log 提醒（非阻断）  
为什么：调试日志常被遗留。  
怎么做：写入 JS/TS 后扫描并提示行号。  
落点：`scripts/hooks/sy-posttool-write.cjs`。  
验证：包含 console.log 的文件触发提示。  
风险：提示过多需节流。

6. 按文件的轻量质量门  
为什么：全量 lint/format 太慢。  
怎么做：只对编辑文件运行格式化或 typecheck，默认提示，严格模式才阻断。  
落点：`scripts/hooks/sy-posttool-write.cjs`。  
验证：标准模式提示、严格模式阻断。  
风险：外部工具缺失时必须跳过。

7. PR/build 后置提示  
为什么：命令完成后用户常忘记下一步。  
怎么做：检测 build 或 `gh pr create`，输出短提示与命令。  
落点：`scripts/hooks/sy-posttool-bash-verify.cjs`。  
验证：命令触发提示。  
风险：提示要短，避免刷屏。

8. 成本记录（token/时间）  
为什么：成本不可见就无法优化。  
怎么做：若 payload 含 usage，写入 `.ai/analysis/costs.jsonl`。  
落点：`scripts/runtime/output-log.cjs`。  
验证：usage 写入成功。  
风险：无 usage 字段时直接跳过。

【避坑指南】
1. hooks 只做 I/O 翻译与调度，业务决策在 Policy Kernel。  
2. stdout 只输出 JSON，所有日志走 stderr。  
3. 并行 hooks 必须无共享状态，必要时串行或用锁。  
4. 新规则先提示后阻断，先放在 strict profile。  
5. Windows 优先 Node 脚本，shell 需要 polyglot wrapper。  
6. hooks 不是 sandbox 的保护对象，按“高权限脚本”管理。

【代码片段】

```json
{
  "decision": "deny",
  "reason": "Blocked: potential secret in content",
  "systemMessage": "Security hook blocked this write"
}
```

---

## 【架构设计亮点补充】（基于深度源码分析）

### 一、Claude Code - 13 事件同步生命周期架构

**核心优势：**
56. **完整的生命周期覆盖**：13 个事件涵盖从 Setup 到 SessionEnd 的完整流程，包括 `PreCompact`（上下文压缩前）、`SubagentStart/Stop`（子代理生命周期）、`PostToolUseFailure`（工具失败后）等细粒度事件
57. **IPC 契约标准化**：JSON stdin/stdout 通信，退出码语义明确（0=继续，1=警告，2=阻断/反馈/强制继续），支持 `additionalContext` 注入模型上下文
58. **输入改写能力**：PreToolUse 可返回修改后的工具输入，支持参数规范化、路径修正、命令重写等场景
59. **PermissionRequest 自动审批**：可处理权限请求并返回 `allow/deny/ask`，支持 `alwaysAllow` 建议并更新权限状态，实现自动化审批流
60. **Stop 事件的双重语义**：exit 2 表示"强制继续"而非阻断，适合 checkpoint 失败时的降级策略；输入包含 `last_assistant_message` 避免读取 transcript
61. **Hook 绑定机制**：支持绑定到 skill/agent/frontmatter，实现细粒度的 hook 作用域控制
62. **once 标记**：`once: true` 限制单次运行，适合启动提示、一次性初始化、会话引导等场景
63. **HTTP Hooks 支持**：允许远程策略服务集成，适合企业安全集成和集中式策略管理（需 fail-open 保证可用性）
64. **Managed Settings 控制**：可通过配置禁用用户或项目 hooks，适合企业环境的安全管控
65. **并行执行策略**：并行 hooks 不保证顺序且互不依赖，需要顺序时必须显式串行化

**实现细节：**
66. **元数据丰富**：输入包含 `hook_event_name`、`agent_id`、`agent_type`、`agent_transcript_path`、`tool_use_id`、`permission_mode` 等，支持多代理审计与工具链追踪
67. **输出收敛展示**：PostToolUse 输出有"收敛展示"策略，可借鉴为 output.log 的"短摘要 + 证据指针"模式
68. **Sandbox 边界明确**：官方说明 sandbox 只作用于 Bash，不覆盖 hooks；hooks 按"高权限脚本"管理
69. **会话启动加载**：hooks 在会话启动时加载，运行中变更不生效，测试必须重启会话
70. **插件 hooks 合并**：插件 hooks 与用户 hooks 会合并并并行执行，需要明确优先级与冲突策略
71. **Flag File 模式**：可用 flag file 方式做"临时启用/禁用"或"只在 CI 执行"，减少日常噪音
72. **Timeout 管理**：hooks timeout 上限较高，需设置合理上限并避免阻塞核心路径
73. **简化模式降级**：`CLAUDE_CODE_SIMPLE` 禁用 hooks，需要在 seeyue 中显式检测并降级为纯审计

### 二、Gemini CLI - 模块化 Hooks 架构

**核心优势：**
74. **四层组件分层**：HookRegistry（注册）、HookPlanner（计划）、HookRunner（执行）、HookAggregator（聚合），职责清晰、易于测试
75. **多源配置系统**：支持 Runtime > Project > User > System > Extensions 五层配置，优先级明确，支持配置合并与覆盖
76. **指纹信任机制**：项目 hooks 会被指纹校验，命令或名称变更会被视为不受信任，需要重新授权
77. **事件特定聚合策略**：不同事件有不同的聚合逻辑（BeforeToolSelection 做 union 合并、AfterAgent 触发重试、Tool 仅阻断该工具）
78. **BeforeModel 强大能力**：可直接替换请求或注入合成响应，支持请求拦截、缓存、mock 等高级场景
79. **AfterTool 输出替换**：可隐藏真实输出并替换 reason，适合敏感信息脱敏、输出规范化等场景
80. **BeforeAgent 输入丢弃**：可丢弃用户输入，适合安全拦截、输入验证失败等场景
81. **ToolConfig 过滤与合并**：支持 `mode: NONE` 强制禁用工具；多 hook whitelist 做 union 合并，灵活控制工具可用性
82. **Sequential/Parallel 控制**：hooks 组级 `sequential` 控制串行，默认并行，适合竞争资源或有依赖的场景
83. **CLI 管理命令**：提供 `/hooks enable-all|disable-all|enable <name>|disable <name>` 和 `/hooks panel`（执行计数与失败原因），交互友好
84. **环境变量脱敏**：环境经过 sanitization，最小化 env 透传并显式允许，降低第三方 hooks 风险
85. **SuppressOutput 选项**：可隐藏 hook 元数据，适合敏感环境和生产部署
86. **Telemetry 脱敏**：hook name 会进入 telemetry，默认脱敏（可选开启完整记录），避免泄露敏感参数
87. **Fail-Open 策略**：stdout 污染会被当作 systemMessage 并默认放行，强调"严格 JSON"与 fail-open 保证可用性
88. **Exit Code 分级**：0=成功解析 JSON，2=阻断，其他=警告继续，错误分级语义明确
89. **基础审计字段**：输入包含 `transcript_path` 与 `timestamp`，适合做统一审计定位

**实现细节：**
90. **BeforeToolSelection 限制**：不支持 `decision`/`continue`/`systemMessage`，只接受 toolConfig，避免语义混淆
91. **同步阻塞模型**：hooks 同步阻塞，CLI 等待所有匹配 hooks 完成；重型逻辑必须缓存并收窄 matcher
92. **配置层级可视化**：`/hooks panel` 可看执行计数与失败原因，配置按 project/user/system/extension 合并
93. **检查点语义**：检查点必须是"变更前快照"，在 mutating tool 之前创建，保证回滚一致性
94. **Policy Engine 分离**：policy engine 负责 allow/ask/deny，hooks 只做证据与状态同步，职责分离

### 三、Codex - Rust 实现的高性能 Hooks

**核心优势：**
95. **类型安全的 Rust 实现**：使用 Rust 实现 hooks 系统，类型安全、性能高、内存安全
96. **HookResult 三态模型**：Success/FailedContinue/FailedAbort 并短路，语义清晰，适合"必须阻断"的硬门
97. **稳定的 Wire Shape**：HookPayload 有稳定 JSON wire shape 测试，固化序列化格式，减少兼容破坏
98. **丰富的审计字段**：AfterToolUse payload 提供 `mutating`、`duration_ms`、`sandbox`、`sandbox_policy`、`output_preview`，适合统一审计
99. **工具类型细分**：HookToolInput 明确区分 Function/Custom/LocalShell/MCP，LocalShell 包含 sandbox 权限、prefix_rule 与 justification
100. **Fire-and-Forget 通知**：通知 hook 采用 argv + JSON 末参的 fire-and-forget 模式，不依赖 stdout/stderr，避免阻塞
101. **顺序执行与短路**：hooks 顺序执行且遇 abort 立刻短路，保证关键 hooks 的执行优先级
102. **固定事件结构**：HookPayload 固定 `event_type` + `event` 结构，并用序列化测试固化，保证向后兼容

**实现细节：**
103. **Hook 覆盖范围**：只覆盖 AfterAgent / AfterToolUse，不可前置阻断；适合审计与通知，不适合硬拦截
104. **Legacy Notify 模式**：legacy notify 走 argv + JSON 末参并 stdio 置空，兼容旧版本 hooks

### 四、Everything Claude Code - Profile 与 Flag 系统

**核心优势：**
105. **Profile Gating 系统**：支持 minimal/standard/strict 三级 profile，配合 `run-with-flags` 做 profile gating 与禁用列表
106. **Async 与 Timeout 支持**：hooks 支持 `async` 与 `timeout`，适合异步检查、远程验证等场景
107. **Stdin 大小限制**：`run-with-flags` 限制 stdin 大小，在禁用/缺失脚本时原样透传，降低失败面
108. **多路径搜索**：SessionStart 通过多路径搜索 plugin root，避免 `CLAUDE_PLUGIN_ROOT` 缺失导致初始化失败
109. **Hook 启用检查**：提供 `check-hook-enabled.js` 检查 hook 是否启用，支持 profile 和禁用列表
110. **Hook Flags 库**：`hook-flags.js` 提供统一的 flag 管理，支持环境变量、配置文件、命令行参数
111. **CI 验证脚本**：`validate-hooks.js` 提供 CI 集成的 hooks 验证，确保 hooks 配置正确性
112. **规则文件热更新**：Hookify 支持规则文件热更新、warn/block 分级、多规则聚合

**实现细节：**
113. **新规则渐进式部署**：新规则先提示后阻断，先放在 strict profile，逐步推广到 standard/minimal
114. **跨平台兼容性**：Windows 优先 Node 脚本，shell 需要 polyglot wrapper（如 `run-hook.cmd`）

### 五、Superpowers - 跨平台稳定性

**核心优势：**
115. **Polyglot Wrapper**：用 polyglot `run-hook.cmd` 解决 Windows bash 缺失与 .sh 自动检测问题
116. **静默降级**：缺 bash 时静默放行，避免 hooks 缺失导致系统不可用
117. **跨平台路径处理**：统一处理 Windows/Unix 路径差异，避免路径分隔符问题

### 六、统一架构设计建议

**基于以上分析的关键改进方向：**

118. **实现分层架构**：借鉴 Gemini CLI 的四层架构（Registry/Planner/Runner/Aggregator），职责清晰、易于测试和扩展
119. **统一 IPC 契约**：定义 seeyue 最小字段集，为缺失字段给出明确降级策略，避免适配器逻辑分散
120. **多源配置系统**：实现 Runtime > Project > User > System 的配置优先级，支持配置合并与覆盖
121. **指纹信任机制**：对项目 hooks 进行指纹校验，变更时需要重新授权，提升安全性
122. **事件特定聚合**：不同事件采用不同的聚合策略，避免"一刀切"的处理逻辑
123. **Profile 系统**：实现 minimal/standard/strict 三级 profile，支持渐进式规则部署
124. **CLI 管理命令**：提供交互式的 hooks 管理命令，提升用户体验
125. **审计字段标准化**：统一审计字段（mutating、duration_ms、sandbox、output_preview 等），支持跨引擎审计
126. **Fail-Open 策略**：关键路径采用 fail-open 策略，保证系统可用性
127. **类型安全实现**：考虑使用 TypeScript 或 Rust 实现核心 hooks 逻辑，提升类型安全性和性能
128. **测试与验证**：提供 hook-linter、validate-hook-schema、test-hook 等工具，确保 hooks 质量
129. **文档与示例**：提供完整的文档、最佳实践、示例代码，降低 hooks 开发门槛
130. **遥测与监控**：集成遥测系统，收集 hooks 执行统计、失败原因、性能指标等

---

【参考文件（追溯）】
1. `refer/agent-source-code/claude-code-main/CHANGELOG.md`
2. `refer/agent-source-code/claude-code-main/examples/hooks/bash_command_validator_example.py`
3. `refer/agent-source-code/claude-code-main/examples/settings/README.md`
4. `refer/agent-source-code/claude-code-main/plugins/plugin-dev/skills/hook-development/references/advanced.md`
5. `refer/agent-source-code/claude-code-main/plugins/plugin-dev/skills/hook-development/references/migration.md`
6. `refer/agent-source-code/claude-code-main/plugins/plugin-dev/skills/hook-development/scripts/README.md`
7. `refer/agent-source-code/claude-code-main/plugins/plugin-dev/skills/hook-development/SKILL.md`
8. `refer/agent-source-code/claude-code-main/plugins/hookify/core/rule_engine.py`
9. `refer/agent-source-code/claude-code-main/plugins/hookify/commands/hookify.md`
10. `refer/agent-source-code/codex-main/codex-rs/hooks/src/types.rs`
11. `refer/agent-source-code/codex-main/codex-rs/hooks/src/registry.rs`
12. `refer/agent-source-code/codex-main/codex-rs/hooks/src/user_notification.rs`
13. `refer/agent-source-code/codex-main/docs/config.md`
14. `refer/agent-source-code/gemini-cli-main/docs/hooks/index.md`
15. `refer/agent-source-code/gemini-cli-main/docs/hooks/reference.md`
16. `refer/agent-source-code/gemini-cli-main/docs/hooks/best-practices.md`
17. `refer/agent-source-code/gemini-cli-main/docs/hooks/writing-hooks.md`
18. `refer/agent-source-code/gemini-cli-main/packages/core/src/hooks/` (完整实现)
19. `refer/superpowers-main/hooks/run-hook.cmd`
20. `refer/everything-claude-code-main/hooks/hooks.json`
21. `refer/everything-claude-code-main/scripts/ci/validate-hooks.js`
22. `refer/everything-claude-code-main/scripts/hooks/run-with-flags.js`
23. `refer/everything-claude-code-main/scripts/hooks/run-with-flags-shell.sh`
24. `refer/everything-claude-code-main/scripts/hooks/check-hook-enabled.js`
25. `refer/everything-claude-code-main/scripts/lib/hook-flags.js`
26. `refer/skills-and-hooks-architecture-advisory.md`
27. `refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md`
28. `refer/v4-architecture-patch-risks.md`
29. `refer/workflow-skills-system-design.md`
30. `refer/workflow-skills-system-design-v3.md`
31. `refer/phase5-integration-summary.md`
32. `refer/output-templates-reference.md`
