# Hooks 开发流程说明

## 目标

本文档聚焦 `seeyue-workflows` 的 hooks 开发顺序与单文件职责，按运行事件顺序给出**逐文件详细说明**。

## 术语说明

| 名词 | 说明 |
|---|---|
| `hooks` / `hook` | 由引擎事件触发的拦截脚本层，用于在执行边界做硬性约束与证据采集。 |
| `Hook Client` | 运行时统一处理入口（`scripts/runtime/hook-client.cjs`），负责封装输入、调用 policy kernel、写入 journal 并输出统一结果。 |
| `hook script` | `scripts/hooks/*.cjs` 下的实际拦截脚本，禁止直接读写 runtime 状态。 |
| `hook event` | 事件名称（如 `SessionStart`、`PreToolUse:Bash`），定义于 `workflow/hooks.spec.yaml`。 |
| `hook contract` | `workflow/hook-contract.schema.yaml`，规范 hook 输入/输出、审批结构与输出 envelope。 |
| `freeze gate` | `workflow/validate-manifest.yaml` 的冻结门控，约束规格在特定节点后不可变更。 |
| `runtime snapshot` | 从 `.ai/workflow/*` 读取的运行态快照，供 hooks 判定使用。 |
| `policy kernel` | `scripts/runtime/policy.cjs`，统一做审批/TDD/风险决策。 |
| `journal` | 运行态事件日志 `.ai/workflow/journal.jsonl`，追加式写入。 |
| `stdout JSON-only` | Hook 输出只允许 JSON，禁止混入非 JSON 文本。 |
| `stdin single-read` | Hook 只读取一次 stdin，避免二次读取导致空输入。 |
| `adapter` | 将 machine spec 编译为引擎侧配置文件的生成器。 |
| `capability-gap` | 引擎缺口报告 `.ai/workflow/capability-gap.json`，用于标注 hooks 覆盖差异。 |
| `SessionStart` | 会话开始事件，注入 bootstrap 约束与上下文。 |
| `UserPromptSubmit` / `BeforeAgent` | 用户提示提交或 agent 执行前事件，用于长会话重锚。 |
| `BeforeToolSelection` | Gemini CLI 的工具选择前事件，用于限制可选工具集合。 |
| `PreToolUse:*` | 工具调用前事件（如 `Bash`、`Write|Edit`），做硬拦截。 |
| `PostToolUse:*` | 工具调用后事件，用于审计与证据采集。 |
| `AfterModel` | Gemini CLI 的模型输出后事件，用于脱敏与检查。 |
| `Stop` / `AfterAgent` | 回合结束事件，用于 checkpoint gate 与恢复校验。 |
| `runHookAndExit` | Hook 脚本统一入口，读取 stdin、分发事件、输出并退出。 |
| `verdict` | Hook 结果：`allow`/`block`/`block_with_approval_request`/`force_continue`。 |
| `block_with_approval_request` | 需要人工审批后继续的阻断类型。 |
| `exitCode=2` | Hook 阻断退出码，触发引擎硬拦截。 |
| `approval_request` | 审批请求的结构化输出，供 UI/系统提示。 |
| `output_templates` | 产出模板实例，写入 `.ai/workflow/output.log`。 |
| `input_mutation` | 对工具输入的合法变更（必要时由 Hook 返回）。 |
| `journal_events` | Hook 需要追加到 journal 的事件列表。 |
| `metadata` | Hook 输出的辅助字段，不影响决策。 |
| `bootstrap` | 会话开始时注入的约束/路由提示。 |
| `sy-workflow` | 路由与工作流约束技能入口。 |
| `sy-constraints` | 约束基线技能，作为强制前置规则。 |
| `context` / `additional_context` | 向引擎注入的补充上下文文本。 |
| `hookSpecificOutput` | 引擎专用输出字段（Gemini/Claude 需要）。 |
| `toolConfig` | Gemini 工具选择配置（模式、白名单）。 |
| `tool_name` / `tool_input` / `tool_response` | Hook 标准输入字段：工具名、输入参数、执行结果。 |
| `command class` | 命令分级（来自 hooks spec 的 `command_classification`）。 |
| `approval_pending` | 运行态存在待审批事项。 |
| `restore_pending` | 运行态处于恢复挂起状态。 |
| `loop budget` | 自动/批量执行预算（节点数、失败数、审批数等）。 |
| `recommended_next` | 运行态给出的下一步建议动作。 |
| `TDD red gate` | TDD 红门：必须先拿到失败测试证据。 |
| `RED evidence` | 失败测试证据（red_recorded）。 |
| `protected files` | 明确禁止直接写入的文件（如 `.env`、lockfile）。 |
| `secret scan` | 高置信凭据扫描与拦截规则。 |
| `placeholder marker` | `TODO/FIXME/HACK` 等占位标记，生产代码禁止残留。 |
| `debug phase` | 系统化调试阶段，未到 phase 5 不允许改代码。 |
| `pre-destructive checkpoint` | 对已有文件或破坏性操作前强制创建检查点。 |
| `session.yaml` / `session.md` | 运行态会话文件（`session.md` 为 legacy）。 |
| `current_phase` / `phase.status` | 会话阶段字段，用于约束状态机合法性。 |
| `run_id` | 运行 ID，格式 `wf-YYYYMMDD-NNN`。 |
| `audit.jsonl` | Hook 写入的审计流水。 |
| `.ai/index.json` | 文件理解索引，写入后标记漂移。 |
| `VERIFY_PASS` | 通过验证的节点标记（审计事件）。 |
| `write_recorded` | 写入动作的 journal 事件。 |
| `test_contract.red_cmd/green_cmd` | 节点级红/绿测试命令约定。 |
| `verify-staging.json` | 验证证据暂存。 |
| `ai.report.json` | 验证汇总报告，用于 review gate。 |
| `red_recorded` / `green_recorded` | TDD 红/绿证据事件。 |
| `verification_recorded` | 验证执行记录事件。 |
| `llm_response` | 模型原始输出（用于 AfterModel 脱敏）。 |
| `redaction` | 对敏感内容替换为 `[REDACTED]` 的过程。 |
| `force_continue` | Stop gate 未通过时的“继续执行”指令。 |
| `ledger.md` | 节点完成的记账文件，用于 review gate。 |
| `next_action` | 会话字段，指向下一步操作。 |
| `Gemini CLI` / `Claude` / `Codex` | 目标引擎/运行平台，hooks 覆盖能力不同。 |

## 开发流程顺序

1. 规范冻结：`workflow/hook-contract.schema.yaml` 定义输入/输出与审批结构；`workflow/hooks.spec.yaml` 定义事件矩阵与引擎覆盖；`workflow/validate-manifest.yaml` 执行冻结门控。
2. Hook Client 实现：`scripts/runtime/hook-client.cjs` 统一 stdin 封装、runtime snapshot、policy kernel 调用、journal 追加与 stdout JSON-only 输出。
3. Hook 脚本实现：`scripts/hooks/*.cjs` 仅做物理拦截与最小逻辑，禁止直接读写 runtime 状态，必须通过 Hook Client。
4. Adapter 输出：`scripts/adapters/compile-adapter.cjs` 生成 `.claude/settings.json`、`.gemini/settings.json`、`.codex/config.toml`，并写出 `.ai/workflow/capability-gap.json`。
5. 验证与回归：`tests/hooks/run-v4-fixtures.cjs`、`tests/hooks/sy-hooks-smoke.cjs`、`tests/e2e/run-engine-conformance.cjs --all`。
6. 发布检查：`docs/release-checklist.md` 与 `sync-manifest.json` 对齐发布与同步边界。

## Hook 运行顺序

1. `SessionStart` → `sy-session-start.cjs`
2. `UserPromptSubmit/BeforeAgent` → `sy-prompt-refresh.cjs`
3. `BeforeToolSelection` → `sy-before-tool-selection.cjs`（Gemini CLI）
4. `PreToolUse:Bash` → `sy-pretool-bash.cjs` + `sy-pretool-bash-budget.cjs`
5. `PreToolUse:Write|Edit` → `sy-pretool-write.cjs` + `sy-pretool-write-session.cjs`
6. `PostToolUse:Write|Edit` → `sy-posttool-write.cjs`
7. `PostToolUse:Bash` → `sy-posttool-bash-verify.cjs`
8. `AfterModel` → `sy-after-model.cjs`（Gemini CLI）
9. `Stop/AfterAgent` → `sy-stop.cjs`

## Hook Client 输出约定

- Hook 脚本统一调用 `runHookAndExit(event)`，标准输出为 JSON-only。
- `verdict` 为 `allow` / `block` / `block_with_approval_request` / `force_continue`。
- `exitCode=2` 表示阻断（Claude/Gemini 触发硬拦截）；`force_continue` 仍返回 `exitCode=0`。
- 可携带字段：`instructions`、`approval_request`、`output_templates`、`input_mutation`、`journal_events`、`metadata`。

## Hooks 文件详解（按运行顺序）

### `scripts/hooks/sy-session-start.cjs`

- 事件：`SessionStart`。
- 输入：stdin JSON（含 `cwd` 等）；`cwd` 默认来自 `payload.cwd` 或 `CLAUDE_PROJECT_DIR`。
- 处理：调用 Hook Client `SessionStart`；若启用，注入 bootstrap 指令，包含 `sy-workflow` 与 `sy-constraints` 路由提示、git 上下文、workflow 状态与 `.ai/index.json` 缺失提示。
- 输出：`additional_context` 与 `hookSpecificOutput.additionalContext`；非阻断。
- 相关开关：`policy.sessionStart.enabled`（默认启用）。

### `scripts/hooks/sy-prompt-refresh.cjs`

- 事件：`UserPromptSubmit` / `BeforeAgent`。
- 输入：stdin JSON（含 `prompt` 或 `message`）。
- 处理：当处于 `plan/execute/review` 且命中触发关键词时，注入约束锚点提示，要求先调用约束类 skills。
- 输出：`context` 字段（非阻断）。
- 相关开关：`SY_BYPASS_PROMPT_REFRESH`，`policy.promptRefresh.triggerKeywords`，`policy.promptRefresh.activePhases`。

### `scripts/hooks/sy-before-tool-selection.cjs`

- 事件：`BeforeToolSelection`（Gemini CLI）。
- 输入：stdin JSON（含 session/runtime 状态）。
- 处理：如 `approval_pending` 或 `restore_pending`，下发工具禁用模式；非执行阶段或解析异常时，仅允许只读工具集合。
- 输出：`hookSpecificOutput.toolConfig`（`mode` 与 `allowedFunctionNames`）。
- 相关开关：`SY_BYPASS_TOOL_SELECTION`，`policy.beforeToolSelection.*`。

### `scripts/hooks/sy-pretool-bash.cjs`

- 事件：`PreToolUse:Bash`。
- 输入：stdin JSON（`tool_name`、`tool_input.command`）。
- 处理：命令黑名单拦截（`git push --force`、`git reset --hard`、`rm -rf`、`.env` 重定向等）；`git commit`/`git push` 必须显式授权；命令分类后走 `policy.evaluate`，需要审批则阻断；对 destructive/privileged 类命令要求预检查点。
- 输出：阻断时 `exitCode=2` 并输出审批指引；允许则 `allow_pretool_bash`。
- 相关开关：`SY_BYPASS_PRETOOL_BASH`、`SY_ALLOW_GIT_COMMIT`、`SY_ALLOW_GIT_PUSH`、`.claude/sy-hooks.policy.json`。

### `scripts/hooks/sy-pretool-bash-budget.cjs`

- 事件：`PreToolUse:BashBudget`。
- 输入：stdin JSON（`tool_input.command`）。
- 处理：识别执行类命令（build/test/lint 等），仅在 `execute` 阶段生效；基于 runtime `loop_budget` 与 `approval/recovery` 状态阻断自动执行。
- 输出：预算耗尽时阻断并提示 `recommended_next`；否则放行。
- 相关开关：`SY_BYPASS_LOOP_BUDGET`、`SY_BYPASS_PRETOOL_BASH`。

### `scripts/hooks/sy-pretool-write.cjs`

- 事件：`PreToolUse:Write|Edit`。
- 输入：stdin JSON（`file_path`、`content/new_string`）。
- 处理：保护文件拦截（`.env`、lockfile 等）；TDD 红门（需 RED 证据）；按 policy 触发写入审批；secret 扫描与硬编码凭据拦截；生产代码禁止 TODO/占位；debug 阶段 <5 禁止写入；修改现有文件前强制预检查点。
- 输出：不满足任一 gate 时 `exitCode=2` 阻断并给出指引。
- 相关开关：`SY_BYPASS_PRETOOL_WRITE`、`SY_BYPASS_SECRET_GUARD`、debug 状态字段。

### `scripts/hooks/sy-pretool-write-session.cjs`

- 事件：`PreToolUse:WriteSession`。
- 输入：stdin JSON（目标写入 `session.yaml`/`session.md` 内容）。
- 处理：校验 `current_phase` 与 `phase.status` 合法值；检测阶段回退并告警；校验 `run_id` 格式；只对 session 文件生效。
- 输出：非法状态阻断；回退仅告警。
- 相关开关：`SY_BYPASS_SESSION_GUARD`、`SY_BYPASS_PRETOOL_WRITE`。

### `scripts/hooks/sy-posttool-write.cjs`

- 事件：`PostToolUse:Write|Edit`。
- 输入：stdin JSON（`file_path`、`content`、`tool_name`）。
- 处理：写入审计 `audit.jsonl`；更新 `.ai/index.json` 指纹；对中等置信度凭据模式给出告警；若写入 session 且 `last_completed_node` 更新则记录 `VERIFY_PASS`；检测执行阶段范围漂移并警告；追加 `write_recorded` journal。
- 输出：始终放行，附带警告信息到 stderr。

### `scripts/hooks/sy-posttool-bash-verify.cjs`

- 事件：`PostToolUse:Bash`。
- 输入：stdin JSON（`tool_input.command`、`tool_response`）。
- 处理：解析退出码与 stdout/stderr 关键特征；匹配 `test_contract.red_cmd/green_cmd` 记录 `red_recorded/green_recorded`；按命令类型写入 `.ai/analysis/verify-staging.json` 并同步 `ai.report.json`；追加 `verification_recorded` journal。
- 输出：始终放行。
- 相关开关：`SY_BYPASS_VERIFY_CAPTURE`。

### `scripts/hooks/sy-after-model.cjs`

- 事件：`AfterModel`（Gemini CLI）。
- 输入：stdin JSON（`llm_response`）。
- 处理：扫描 `llm_response` 中高置信 secrets，最多执行 `maxRedactions` 次替换为 `[REDACTED]`；返回红线统计信息。
- 输出：`hookSpecificOutput.llm_response` 为脱敏后的响应；不阻断。
- 相关开关：`SY_BYPASS_AFTER_MODEL`、`SY_BYPASS_SECRET_GUARD`。

### `scripts/hooks/sy-stop.cjs`

- 事件：`Stop` / `AfterAgent`。
- 输入：stdin JSON（默认读取 runtime 状态）。
- 处理：加锁避免并发；若 `restore_pending` 或 `approval_pending` 直接 `force_continue` 并输出阻断原因与指令；按阶段检查 `audit.jsonl`、`ai.report.json`、`ledger.md` 与 `next_action`；若检查失败则 `force_continue`。
- 输出：满足检查则允许停止；未满足则提示修复与恢复路径。
- 相关开关：`SY_BYPASS_STOP_GUARD`。

## 基础设施文件（同样属于 hooks 目录）

### `scripts/hooks/gemini-hook-bridge.cjs`

- 角色：Gemini CLI hooks 入口桥接层。
- 输入：命令行参数 `--mode` 与 `--delegate`；stdin 为 Gemini hook payload。
- 处理：标准化 payload 字段（`tool_name`/`tool` 等）；调用 delegate hook；将 exitCode=0/2 映射为 Gemini `decision` 或 `systemMessage`，并回写 `hookSpecificOutput`。
- 输出：Gemini 期望的 JSON 格式（`decision`、`hookSpecificOutput`、`systemMessage`）。

### `scripts/hooks/sy-hook-lib.cjs`

- 角色：所有 sy-* hooks 的共享库与默认策略定义。
- 功能：策略深度合并（数组拼接）、stdin 读取与 JSON 解析、文件路径与写入内容归一、secret 扫描、workflow/session 解析、debug 阶段读取、ledger 统计、scope 判断。
- 默认策略覆盖：命令黑名单、保护文件列表、secret 规则、stop gate 参数、prompt refresh 关键词、tool selection 白名单、AfterModel 脱敏阈值。
- 策略来源：`SY_HOOKS_POLICY` 或 `.claude/sy-hooks.policy.json`。
