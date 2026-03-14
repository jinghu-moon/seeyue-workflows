# seeyue-workflows 项目介绍

## 一句话定位

`seeyue-workflows` 是一个独立维护的工作流控制平面仓库，专门沉淀可复用的 `skills + hooks + runtime + specs`，为 Claude Code、Codex、Gemini CLI 等引擎提供一致的可验证工作流能力。

## 项目解决的问题

- 把“工作流规则”从业务代码中剥离，形成可复用、可审计、可同步的控制平面。
- 让多引擎的行为边界一致，避免不同工具之间的审批、TDD、验证行为漂移。
- 把运行态与验证证据标准化，降低“口头完成”和“未验证提交”的风险。

## 核心设计理念

- 状态优先：以 `.ai/workflow/*` 为事实源，避免依赖聊天上下文推断执行状态。
- 可验证优先：输出模板、验证报告与日志均可机器解析。
- 安全优先：写入、命令、审批、恢复等关键动作都有硬门控。

## 核心组成

### 1. 规范层（machine source of truth）

- 规范文件集中在 `workflow/*.yaml`，例如 `runtime.schema.yaml`、`router.spec.yaml`、`policy.spec.yaml`、`skills.spec.yaml`、`hook-contract.schema.yaml`。
- `workflow/validate-manifest.yaml` 负责冻结门控与规范注册，防止“未冻结先实现”。

### 2. 运行态存储

- 运行态文件位于 `.ai/workflow/`，核心包含 `session.yaml`、`task-graph.yaml`、`sprint-status.yaml`、`journal.jsonl`、`output.log`、`ledger.md`。
- 这些文件由 runtime 读写，作为路由、审核、恢复与验证的依据。

### 3. Runtime 服务

- `router.cjs`：纯路由决策，输出推荐下一步与事件列表。
- `policy.cjs`：统一门控，处理审批、TDD、超时、预算等策略。
- `engine-kernel.cjs`：汇总路由与策略输出，形成可执行决策。
- `controller.cjs`：驱动 run/resume/verify 流程。
- `checkpoints.cjs`、`recovery-bridge.cjs`：恢复与回滚边界。

### 4. Hooks 与 Hook Client

- hooks 脚本位于 `scripts/hooks/`，都是薄封装，统一委派给 `scripts/runtime/hook-client.cjs`。
- Hook Client 负责读取输入、加载快照、调用策略、写入日志、返回统一 verdict。
- 关键 hooks 包含 `PreToolUse:Write|Edit`、`PreToolUse:Bash`、`PostToolUse:Write|Edit`、`PostToolUse:Bash`、`BeforeToolSelection`、`AfterModel`、`Stop`。

### 5. Skills 体系

- `workflow/skills.spec.yaml` 描述技能元信息与触发条件。
- 技能正文存放在 `.agents/skills/`，适配器只注入“stub”，按需加载正文。

### 6. 适配器与编译产物

- 适配器脚本在 `scripts/adapters/`，支持 Claude Code、Codex、Gemini CLI。
- `compile-adapter.cjs` 生成 routing、skills、policy 三类产物，并维护 `AGENTS.md`、`CLAUDE.md`、`GEMINI.md` 等引擎入口文件。

### 7. 输出契约与日志

- 输出模板定义在 `workflow/output-templates.spec.yaml`。
- Hook Client 会校验模板并写入 `.ai/workflow/output.log`，用于回放与审计。

## 运行流程概览

1. `bootstrap-run` 初始化运行态与 task graph。
2. `engine-kernel` 汇总 validator、policy、router 输出，生成下一步决策。
3. `controller` 执行 run/resume/verify，更新运行态与日志。
4. hooks 在关键写入或命令执行前后触发，硬性约束边界。
5. 所有关键事件以 `journal.jsonl` 记录，可回放、可审计。

## 目录结构速览

- `workflow/`：所有机器规格与规范。
- `scripts/runtime/`：运行时引擎、策略、路由、恢复与验证。
- `scripts/hooks/`：引擎 hook 脚本入口。
- `scripts/adapters/`：多引擎适配与编译产物。
- `.agents/skills/`：技能正文。
- `tests/`：运行时、hooks、policy、router、output 的回归验证。
- `docs/`：架构、实施计划与操作文档。

## 快速使用

### 在本仓库开发与验证

1. 修改 `workflow/*.yaml` 或 `scripts/runtime/*`。
2. 运行验证命令，确保路由、策略与 hooks 通过回归。
3. 通过适配器输出更新引擎入口产物。

### 在业务仓库同步

1. 通过 `scripts/sync-workflow-assets.py` 同步资产到业务仓库。
2. 在业务仓库启用 hooks 配置，例如 `.claude/settings.json` 与 `.claude/sy-hooks.policy.json`。
3. 在业务仓库运行 hooks 与 runtime 测试，确保集成成功。

## 常用命令

- `npm run test:hooks:smoke`
- `npm run test:skills:core`
- `npm run test:skills:constraints`
- `npm run test:runtime:p2`
- `npm run test:output:log`
- `npm run runtime:run`
- `npm run runtime:verify:write-report`

## 运行环境要求

- Node.js `>=22`
- Python `>=3.11`

## 当前版本与路线

- 当前主线以 V4 规范与实现为准，V5 作为加固与编译链路升级计划存在于 `docs/architecture-v5-proposal.md` 与 `docs/implementation-plan-v5.md`。
- 所有执行行为以 `workflow/*.yaml` 为最终事实源。

## 许可证

- 本项目采用 `AGPL-3.0-or-later`，详见 `LICENSE` 与 `NOTICE`。
