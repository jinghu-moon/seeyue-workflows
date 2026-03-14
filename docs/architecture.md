# seeyue-workflows 架构文档

本文件面向新贡献者与维护者，基于 `docs/project-overview.md` 的定位与仓库代码实现编写。它不是概念草案，而是对当前实现的“事实性说明”。当描述与代码不一致时，以实现为准。

## 1. Project Overview

`seeyue-workflows` 是一个独立维护的工作流控制平面仓库，目标是把“工作流规范、策略、运行态与审计”从业务代码中抽离出来，并为 Claude Code、Codex、Gemini CLI 等引擎提供一致、可验证、可复用的执行框架。它不直接解决业务问题，而是提供一个“执行规则”的硬壳：以 `workflow/*.yaml` 为机器真源，以 `.ai/workflow/*` 为运行态事实，以 runtime 的路由与策略为唯一决策入口。

你可以把它理解为“工作流的中控台”：它不写业务代码，但决定什么时候可以写、如何验证、何时需要人类审批以及如何把证据落盘。

## 2. Architecture Overview

系统由四条主线构成：规范层、运行层、适配层、验证层。

```text
┌────────────────────┐
│ workflow/*.yaml    │  机器真源：路由/策略/技能/Hook/状态结构
└─────────┬──────────┘
          │ load + validate
┌─────────▼──────────┐
│ runtime            │  engine-kernel + router + policy + controller
│ scripts/runtime/*  │  读写 .ai/workflow/* 生成决策与证据
└─────────┬──────────┘
          │ hooks / adapters
┌─────────▼──────────┐
│ hooks              │  scripts/hooks/* -> hook-client
│ adapters           │  compile-adapter -> CLAUDE.md / AGENTS.md / GEMINI.md
└─────────┬──────────┘
          │ persistence
┌─────────▼──────────┐
│ .ai/workflow/*     │  session/task-graph/sprint-status/journal/ledger
└────────────────────┘
```

在这条主线上，`workflow/*.yaml` 约束“可以做什么”，`runtime` 决定“下一步做什么”，`hooks` 把规则下沉到执行边界，`adapters` 把规则变成各引擎可消费的入口文件与设置。

## 3. Design Philosophy

核心设计思想来自三个关键问题：如何避免“聊天误导执行”、如何防止“未验证就提交”、如何让多引擎行为一致。

- 状态优先。`workflow/runtime.schema.yaml` 明确了 `.ai/workflow/*` 的结构，运行时只依赖“可复盘状态”，不依赖聊天历史。
- 规则可验证。`scripts/runtime/spec-validator.cjs` 将规范当成硬约束，`validate-manifest.yaml` 负责冻结门控。
- 策略先于路由。`scripts/runtime/policy.cjs` 先给出审批/TDD/验证门禁，`scripts/runtime/router.cjs` 再基于策略输出 `recommended_next`。
- 证据优先。验证结果通过 `.ai/analysis/ai.report.json`、`.ai/workflow/journal.jsonl`、`.ai/workflow/output.log` 固化。
- 单一活跃执行单元。V4 只支持单活跃 phase 与单活跃 node，避免并行状态漂移。
- 多引擎一致性。`scripts/adapters/compile-adapter.cjs` 统一生成各引擎入口与桥接配置，避免规则在引擎间漂移。

## 4. Core Concepts

以下概念贯穿所有模块。

| 概念 | 说明 | 关键文件 |
| --- | --- | --- |
| Run | 一次工作流会话，绑定 `run_id` | `workflow/runtime.schema.yaml`、`.ai/workflow/session.yaml` |
| Phase | 阶段（plan/execute/review） | `workflow/runtime.schema.yaml`、`.ai/workflow/task-graph.yaml` |
| Node | 任务节点，描述行动与验证 | `workflow/runtime.schema.yaml`、`.ai/workflow/task-graph.yaml` |
| Persona | 执行角色（author/reviewer 等） | `workflow/persona-bindings.yaml` |
| Capability | 抽象能力（code_edit 等） | `workflow/capabilities.yaml` |
| Route Verdict | 路由结果（advance/hold/block/…） | `workflow/router.spec.yaml`、`scripts/runtime/router.cjs` |
| Recommended Next | 统一的下一步结构化动作 | `workflow/router.spec.yaml`、`.ai/workflow/sprint-status.yaml` |
| Approval | 审批请求与授予 | `workflow/policy.spec.yaml`、`workflow/approval-matrix.yaml` |
| Evidence | 测试与验证证据 | `scripts/runtime/verification-evidence.cjs`、`.ai/analysis/ai.report.json` |
| Hooks | 执行边界的强制门控 | `workflow/hooks.spec.yaml`、`scripts/runtime/hook-client.cjs` |

## 5. System Components

系统组件按职责拆分如下。

- 规范层：`workflow/*.yaml` 记录路由、策略、技能、Hook、状态结构与审批矩阵。
- 运行层：`scripts/runtime/*` 实现引擎核心、路由、策略、状态读写、恢复与验证。
- Hook 层：`scripts/hooks/*` 为引擎 hook 入口，统一委派给 `scripts/runtime/hook-client.cjs`。
- 适配层：`scripts/adapters/*` 编译并输出引擎入口文件与配置。
- 证据层：`.ai/workflow/*` 持久化运行态，`.ai/analysis/*` 落盘验证报告。
- 测试层：`tests/*` 以 fixture 方式验证路由、策略、Hook 与输出契约。

## 6. Runtime Engine

运行时以 `engine-kernel -> controller -> transition-applier` 为主路径。

- `scripts/runtime/engine-kernel.cjs`：读取运行态与规范，产出 `validator_verdict + policy_verdict + router_verdict`。
- `scripts/runtime/controller.cjs`：执行 `run/resume/verify`，串起回放、写报告、自动循环与状态落盘。
- `scripts/runtime/transition-applier.cjs`：根据决策更新 `.ai/workflow/*`，并写入 `journal.jsonl` 与 `ledger.md`。

关键数据流片段如下：

```js
const policyVerdict = evaluatePolicy({ session, taskGraph, actionContext, specs });
const routerVerdict = evaluateRouter({ session, taskGraph, sprintStatus, validatorVerdict, policyVerdict, specs });
return { validator_verdict: validatorVerdict, policy_verdict: policyVerdict, ...routerVerdict };
```

这一段说明了“策略先于路由”的硬依赖。router 不会直接读聊天上下文，而是通过 policy 与 runtime state 推导。

## 7. Workflow Specification System

规范系统负责“定义标准并验证偏差”。它有三层。

- 规范本体：`workflow/runtime.schema.yaml`、`workflow/router.spec.yaml`、`workflow/policy.spec.yaml`、`workflow/hooks.spec.yaml`、`workflow/skills.spec.yaml`、`workflow/output-templates.spec.yaml`。
- 规范注册：`workflow/validate-manifest.yaml` 记录 freeze gate 与 cross refs。
- 规范校验：`scripts/runtime/spec-validator.cjs` 与 `scripts/runtime/validate-specs.cjs`。

`spec-validator` 不只是语法检查，它还验证冻结门控是否满足，以及规范之间的交叉引用是否完整。这保证了“未冻结先实现”的行为可以被工具阻断。

## 8. Router and Policy Engine

策略与路由的关系是“策略先判定边界，路由再决定动作”。

### Policy Engine

`policy.cjs` 负责生成以下核心结论。

- 审批结论：由 `workflow/approval-matrix.yaml` 与 `workflow/file-classes.yaml` 合并计算。
- 测试门禁：包含 red/green/behavior/coverage 四类 gate。
- 执行韧性：retry 与 timeout 策略。
- 完成门禁：node/phase/stop gate 的判定。

当红灯未通过时，policy 会返回 `route_effect=block`，并生成原因，强制 hook 与 router 停止推进。

### Router Engine

`router.cjs` 负责产出结构化 `recommended_next`、路由原因与事件列表。它的行为包含：

- 全局阻塞：invalid_state、approval_pending、restore_pending、budget_exhausted。
- Review 交接：spec_reviewer -> quality_reviewer。
- 节点推进与 bypass：通过条件表达式决定是否跳过节点，并记录 `node_bypassed`。

核心判断片段如下：

```js
if (validatorBlocks(validatorVerdict)) {
  return buildResult({ route_verdict: "block", block_reason: "invalid_state", ... });
}
if (session?.approvals?.pending === true) {
  return buildResult({ route_verdict: "block", block_reason: "approval_pending", ... });
}
```

这段逻辑解释了路由器如何优先处理“无效运行态”和“审批阻塞”。

## 9. Hooks Architecture

Hooks 是系统的“边界拦截器”，用于阻断高风险动作，并把证据写入运行态。

- 规范入口：`workflow/hooks.spec.yaml` 定义事件矩阵与命令分类规则。
- IPC 契约：`workflow/hook-contract.schema.yaml` 约束输入输出字段与 verdict。
- 执行入口：`scripts/hooks/*.cjs` 都是薄封装，统一调用 `hook-client`。
- 业务核心：`scripts/runtime/hook-client.cjs` 负责读取输入、加载运行态、应用策略、写日志与输出模板。

Hook 入口示例：

```js
const { runHookAndExit } = require("../runtime/hook-client.cjs");
runHookAndExit("PreToolUse:Write|Edit");
```

Hook Client 还负责：

- 读取 `.ai/workflow/session.yaml` 与 `task-graph.yaml` 并进行快照校验。
- 解析工具输入并识别文件类别、命令类别、风险类别。
- 写入 `.ai/workflow/output.log` 并校验输出模板。
- 对 Stop 事件进行“检查点未完成”的阻断。

## 10. Skills System

Skills 是人类可读指令的“模块化单元”，但它们不被直接写进 adapter 输出，而是通过 stub 引用。

- 注册表：`workflow/skills.spec.yaml`。
- 技能正文：`.agents/skills/*/SKILL.md`。
- Stub 输出：`scripts/adapters/compile-adapter.cjs` 产出 skill stubs，并写入引擎入口文件。
- Manifest：`.ai/workflow/skills-manifest.json` 记录 registry hash，用于变更检测。

这套机制的价值是：技能可迭代，但引擎入口只持有“引用和规则”，避免频繁写入大段文档引发漂移。

## 11. Adapter & Multi-Engine Integration

适配层把统一的 workflow 规则转换成不同引擎的可消费入口。

- 编译器：`scripts/adapters/compile-adapter.cjs`。
- 引擎渲染：`scripts/adapters/claude-code.cjs`、`scripts/adapters/codex.cjs`、`scripts/adapters/gemini-cli.cjs`。
- 验证器：`scripts/adapters/verify-adapter.cjs` 用于检查生成文件与期望一致。

输出产物包括：

- `CLAUDE.md`：Claude Code 指令入口。
- `AGENTS.md`：Codex 指令入口。
- `GEMINI.md`：Gemini CLI 指令入口。
- `.claude/settings.json`、`.codex/config.toml`、`.gemini/settings.json`：Hook 与上下文设置。
- `.ai/workflow/capability-gap.json`：Hook 支持矩阵差距报告。

适配器的重要特性是“同一套 specs，按引擎能力裁剪”。例如 Codex 不支持原生 hooks 时，适配器会把关键事件标记为 bridged。

## 12. Runtime State & Persistence

运行态数据全部落在 `.ai/workflow/*`，其结构由 `workflow/runtime.schema.yaml` 约束。

核心资产包括：

- `session.yaml`：当前 run 的全局状态。
- `task-graph.yaml`：阶段与节点定义，包含 TDD、审批、验证字段。
- `sprint-status.yaml`：路由输出的快照，与 `recommended_next` 对齐。
- `journal.jsonl`：追加式事件日志，用于审计与回放。
- `ledger.md`：人类可读的摘要索引。
- `capsules/`：上下文压缩产物。
- `checkpoints/`：恢复点与快照。
- `output.log`：结构化输出模板日志。

`store.cjs` 对这些文件执行原子写入与锁保护，保证在并发场景中不破坏日志完整性。

## 13. Execution Lifecycle

系统执行生命周期按以下顺序推进。

1. `bootstrap-run` 初始化 `session.yaml`、`task-graph.yaml` 与 `sprint-status.yaml`，并归档旧 run。
2. `controller --mode run` 进入正常执行循环，调用 `engine-kernel` 生成决策。
3. `transition-applier` 写入状态与 `journal.jsonl`，必要时创建 checkpoint。
4. Hooks 在 PreToolUse/ PostToolUse/ Stop 事件中执行门禁与证据收集。
5. `controller --mode verify` 读取 `.ai/analysis/ai.report.json` 生成验证结论。
6. `review-resolution` 写入评审 verdict 并触发下一步路由。
7. `approval-resolution` 在审批完成后刷新路由，进入下一节点或交接。
8. `Stop` hook 作为最后门禁，确保证据齐全再允许停止。

## 14. Developer Workflow

典型开发流程如下。

1. 修改规范：在 `workflow/*.yaml` 调整规则，并运行 `node scripts/runtime/validate-specs.cjs` 验证。
2. 修改 runtime：在 `scripts/runtime/*` 更新逻辑，确保与 specs 一致。
3. 运行回归：使用 `npm run test:runtime:p2` 或针对性测试。
4. 生成产物：运行适配器生成入口文件或验证现有入口文件一致性。
5. 同步到业务仓库：运行 `python scripts/sync-workflow-assets.py --target-root <repo>`。

## 15. Testing Strategy

测试体系覆盖三个层级：规范验证、核心 runtime 行为、引擎适配输出。

- 规范与运行态：`tests/runtime/*` 包含 router/policy/kernel/transition/repair 的 fixture。
- Hooks：`tests/hooks/run-v4-fixtures.cjs`、`tests/hooks/sy-hooks-smoke.cjs`。
- Output：`tests/output/run-output-log-fixtures.cjs` 与模板回归。
- 适配器：`tests/adapters/*` + `scripts/adapters/verify-adapter.cjs`。
- E2E：`tests/e2e/run-engine-conformance.cjs`。

可直接使用 `package.json` 中的脚本，例如 `npm run test:runtime:controller`、`npm run test:output:log`。

## 16. Repository Structure

主要目录结构如下。

- `workflow/`：机器真源规范。
- `scripts/runtime/`：运行引擎与核心逻辑。
- `scripts/hooks/`：Hook 脚本入口与共享库。
- `scripts/adapters/`：适配器编译与输出。
- `.agents/skills/`：技能正文。
- `tests/`：各模块回归测试。
- `docs/`：架构、计划与操作文档。
- `.ai/`：运行态输出目录（运行时生成）。

## 17. Example Execution Flow

以下示例展示一次“修复 bug”流程的关键轨迹。

1. 运行 `bootstrap-run` 初始化 session 与 task graph，`recommended_next` 指向第一个 ready node。
2. `controller --mode run` 计算出 `start_node`，`transition-applier` 将该 node 标记为 `in_progress` 并写入 `journal.jsonl`。
3. 开发者触发写入时，`PreToolUse:Write|Edit` hook 检查 TDD red gate 与文件类别。
4. 测试执行后，`PostToolUse:Bash` 收集 green/coverage evidence，写入 `.ai/analysis` 与 `output.log`。
5. `controller --mode verify` 判断验证完成，节点进入 review 状态。
6. `review-resolution` 记录 spec/quality review 结论，并触发下一步。
7. 所有 gate 满足后，`router` 生成 `session_stopped`，`Stop` hook 放行。

这条路径的关键点是：每一步都在 `.ai/workflow` 留痕，且由 `recommended_next` 驱动。

## 18. Extension Guide (Add Skills / Hooks / Policies)

### 新增 Skill

- 在 `workflow/skills.spec.yaml` 注册 skill，定义 entry 与输出模板。
- 在 `.agents/skills/<skill>/SKILL.md` 创建正文。
- 如需同步到业务仓库，更新 `sync-manifest.json`。
- 运行适配器编译，确保入口文件更新。

### 新增 Hook

- 在 `scripts/hooks/` 添加 hook 脚本并调用 `runHookAndExit`。
- 在 `workflow/hooks.spec.yaml` 的 `hook_matrix` 中注册事件与脚本。
- 若新增事件，更新 `hook-contract.schema.yaml` 与 Hook Client dispatch。

### 调整 Policy

- 在 `workflow/policy.spec.yaml` 修改规则与门禁语义。
- 如需新增审批矩阵或文件类别，更新 `workflow/approval-matrix.yaml` 与 `workflow/file-classes.yaml`。
- 使用 `tests/policy/run-policy-fixtures.cjs` 验证变更。

## 19. Integration Guide for External Projects

对外集成的关键步骤如下。

1. 在目标仓库运行 `python scripts/sync-workflow-assets.py --target-root <target>`，同步 hooks、skills、settings。
2. 确认 `.claude/settings.json`、`.codex/config.toml` 或 `.gemini/settings.json` 已加载对应 hook 配置。
3. 在目标仓库执行回归命令，例如 `node tests/hooks/run-v4-fixtures.cjs`。
4. 对于 Codex，确保 `AGENTS.md` 与 `.codex/config.toml` 同步更新。
5. 对于 Claude Code/Gemini CLI，确保 hook 脚本路径可达并具备执行权限。

## 20. Troubleshooting & Debugging

常见问题与排查路径。

- `invalid_state`：检查 `session.yaml`、`task-graph.yaml` 是否缺字段，必要时运行 `runtime:repair`。
- `approval_pending`：用 `scripts/runtime/approval-resolution.cjs` 写入审批结果。
- `restore_pending`：查看 `.ai/workflow/checkpoints` 是否完整，必要时执行恢复。
- `spec validation fail`：运行 `node scripts/runtime/validate-specs.cjs --all` 查看具体报错。
- `Stop 被阻断`：查看 `output.log` 与 `journal.jsonl` 是否缺失验证证据。

## 21. Security & Guardrails

系统的安全与门禁主要依赖以下机制。

- 命令分类：`workflow/hooks.spec.yaml` 定义 destructive / git_mutating / network_sensitive 等规则。
- 文件分类：`workflow/file-classes.yaml` 定义 system/security/secret 边界。
- 审批矩阵：`workflow/approval-matrix.yaml` 绑定风险等级与审批模式。
- TDD 门禁：`policy.cjs` 在 red/green 未验证时阻断写入。
- Stop Gate：`hook-client` 强制在缺证据时阻断停止。
- Secret 扫描：`scripts/hooks/sy-hook-lib.cjs` 内置敏感信息扫描与占位符判定。

## 22. Future Roadmap

当前实现以 V4 规范为准，V5 作为强化方向存在于 `docs/architecture-v5-proposal.md` 与 `docs/implementation-plan-v5.md`。未来重点包括：

- Hook 扩展为更完整的 registry/runner 机制。
- 更细化的 spec freeze gate 与 validator 分层。
- 多引擎能力差距的自动补偿策略（capability gap 的自动桥接）。
- 运行态更多“可恢复”策略，包括更强的 checkpoint metadata。
- 更完整的 MCP/外部服务集成管道。

---

本文档基于当前仓库实现编写，如需了解单个模块的代码细节，请优先查看 `scripts/runtime/*` 与 `workflow/*.yaml`。如有新的执行规范，请先更新规范，再更新实现。
