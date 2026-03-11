# V4 严格自我评审（2026-03-09）

## 1. 评审范围

本次评审对象：

- `docs/architecture-v4.md`
- `workflow/runtime.schema.yaml`
- `workflow/router.spec.yaml`
- `workflow/policy.spec.yaml`
- `scripts/runtime/*`
- `scripts/hooks/*`
- `.ai/workflow/*`
- `.ai/analysis/ai.report.json`

本次评审采用“证据优先”方式，结论同时参考：

- 仓库本地实现与运行态证据
- `refer/` 目录中的优秀工作流、agent、skills 设计
- 官方公开资料中的 hooks、sub-agents、durable execution、instruction layering、checkpointing 思路

## 2. 最终结论

**Verdict：`CONCERNS`**

一句话结论：

> `V4` 的方向是正确的，控制面已经相当成熟，但距离“production-grade autonomous workflow engine”还差最后一层统一执行闭环。

需要特别区分两个概念：

- `.ai/analysis/ai.report.json = READY`：表示验证阶段通过，可以进入 review。
- review verdict = `CONCERNS`：表示架构方向可继续推进，但仍存在需要优先补齐的系统性缺口。

也就是说：**READY 不等于 PASS**。

## 3. 新鲜验证证据

本次评审前重新执行了以下命令，结果均通过：

```bash
npm run test:hooks:smoke
npm run test:runtime:p2
npm run test:runtime:context
npm run test:runtime:recovery
npm run test:e2e:engine-conformance
npm run test:e2e:doc-link-check
npm run test:e2e:release
'{}' | node scripts/hooks/sy-stop.cjs
```

运行态证据：

- `.ai/workflow/session.yaml` 当前处于 `phase.status=review`
- `.ai/workflow/sprint-status.yaml` 当前 `recommended_next -> workflow-review`
- `.ai/analysis/ai.report.json` 当前 `overall=READY`

## 4. 做对了什么

### 4.1 大方向对齐主流 agent 设计

`docs/architecture-v4.md` 已明确吸收以下成熟设计脉络：

- Codex 风格的分层指令与作用域继承
- Gemini 风格的 durable state over chat
- Claude Code 风格的 hooks / interrupt boundary
- Superpowers 风格的 fresh subagent per task + 双阶段 review
- Everything-Claude-Code 风格的 research-first / verification-first / TDD gate

这不是“像不像”的问题，而是架构抽象已经基本站在正确轨道上。

### 4.2 机器事实源定义正确

V4 已把逻辑规范与运行态分清：

- `workflow/*.yaml`：逻辑真相源
- `.ai/workflow/*`：运行态真相源
- `docs/*.md`：给人看的解释层

这比大量 agent 项目常见的“把聊天上下文当状态机”要稳健得多。

### 4.3 TDD / 假失败 / 遗留系统补丁覆盖率处理成熟

`policy.spec` 与 `policy.cjs` 已经把以下问题处理得比较系统：

- RED 只能接受“行为缺失型失败”，拒绝环境故障、导入错误、语法错误
- GREEN / behavior gate / coverage gate 分离，不让 coverage 冒充行为正确
- 对 legacy code 采用 patch coverage / delta coverage，而不是强迫整个旧文件一次性补齐

这部分已经明显优于很多只写“先测后码”口号、但没有物理门的 workflow 设计。

### 4.4 Router 已具备 production 级规范雏形

`router.spec.yaml` 里已经具备：

- phase / node 两级状态机
- 失败后的恢复出口
- `recommended_next` 的 machine-readable schema
- persona -> capability 路由
- retry / timeout / conditional node 预留位

这说明 V4 已经不是 prompt 集合，而是在逼近真正的 workflow control plane。

## 5. 主要问题

### 5.1 High：缺少统一执行控制器（controller / orchestrator）

**证据**

- `scripts/runtime/engine-kernel.cjs` 目前本质上是“读状态 -> 跑 validator/policy/router -> 返回 verdict”。
- `runEngineKernel` 当前主要被 `tests/runtime/run-engine-kernel.cjs` 消费。
- `package.json` 中没有 `workflow:run`、`workflow:resume`、`workflow:verify` 一类正式运行命令。

**影响**

当前仓库已经有很好的内核部件，但仍偏“可测试组件集合”。
它还不是一个能从 session 启动、持续推进 phase/node、处理中断、落盘事件、再恢复执行的统一控制器。

**建议修复**

新增正式执行入口，例如：

- `scripts/runtime/controller.cjs`
- `node scripts/runtime/controller.cjs --root . --mode run`
- `node scripts/runtime/controller.cjs --root . --mode resume`
- `node scripts/runtime/controller.cjs --root . --mode verify`

### 5.2 High：缺少统一 transition applier，事件与状态更新没有完全收口

**证据**

- `engine-kernel` 会返回 `emit_events`，但没有看到统一的提交器负责把 route 结果、session 更新、task-graph 更新、journal 事件、sprint-status 同步当成一次有边界的状态推进。
- `store.cjs` 具备 atomic write，但更像文件级原子写，而不是“workflow transition transaction”。

**影响**

在单步测试中问题不明显，但在长链路自动执行中，容易出现：

- route 已算出，状态未完全提交
- journal 写了，session 没写
- sprint-status 更新了，task-graph 没更新

这会直接影响 crash recovery 与审计可靠性。

**建议修复**

新增 transition 层，例如：

- `scripts/runtime/transition-applier.cjs`
- 输入：`validator/policy/router` verdict + 当前运行态
- 输出：统一落盘后的新运行态 + journal event bundle

### 5.3 Medium：最终 READY 报告仍偏“skill + hook 协作生成”，不是 runtime 原生命令

**证据**

- `scripts/hooks/sy-posttool-bash-verify.cjs` 明确说明：hook 负责采集验证证据；`verification-before-completion` skill 负责写 `ai.report.json`。
- 当前 `.ai/analysis/ai.report.json` 可用，但它更像“技能驱动的最终组装产物”。

**影响**

现在的 verify 流程已经可用，但还不是 runtime-first。
一旦未来希望把 review、CI、engine adapter 统一接到同一个执行入口，就会出现“验证在 skill 层，状态在 runtime 层”的双中心问题。

**建议修复**

把最终报告生成下沉为 runtime 正式命令，例如：

- `node scripts/runtime/controller.cjs --root . --mode verify --write-report`

skill 可以保留，但变成“调用 runtime verify 命令”的上层编排，而不是直接承担最终报告组装职责。

### 5.4 Medium：并行是 schema 级预留，不是 runtime 级落地能力

**证据**

- `router.spec.yaml` 仍明确 `parallel_phases_supported: false`
- `router.spec.yaml` 仍明确 `parallel_nodes_supported: false`
- 但 node schema 已经预留 `parallel_group`

**影响**

这不是 bug，但会让使用者误判“并行已经准备好”。

**建议修复**

二选一：

1. V1 明确写死单活执行，不暴露并行术语给人造成预期偏差。
2. 正式实现 `parallel_group + join barrier + budget partition`。

当前更推荐先做第 1 种，先把单活控制器闭环做实。

### 5.5 Medium：coverage policy 已有，但缺少统一 coverage adapter 输出

**证据**

- `policy.spec.yaml` 已有完整 coverage profile / patch coverage 规则。
- `policy.cjs` 也已消费 coverageEvidence。
- 但 `.ai/analysis/ai.report.json` 当前 coverage 仍为 `n/a`。

**影响**

这会让最终 review 与 release 阶段仍然需要人工解释“为什么这里是 n/a”。

**建议修复**

为不同语言/框架定义统一 coverage adapter 契约，例如：

- `scripts/runtime/coverage-adapter.cjs`
- 标准输出字段：`actual` / `required` / `mode` / `touchedRegionRegressed` / `globalRegressed`

## 6. 为什么不是 REWORK

因为目前的问题主要集中在“执行闭环没有完全打通”，不是“架构理念错误”。

如果是以下情况，我会给 `REWORK`：

- 仍把 chat history 当 source of truth
- 没有 durable state
- 没有 TDD hard gate
- 没有 review 分层
- 没有恢复 / checkpoint 设计

但 V4 恰恰已经把这些最难的方向做对了。

所以当前最合理的判断不是推翻，而是：

> **保留 V4 主体设计，优先补 controller / transition / verify 三个闭环点。**

## 7. 对照参考来源

### 7.1 本地 refer 证据

- `refer/everything-claude-code-main/AGENTS.md`
- `refer/everything-claude-code-main/rules/common/testing.md`
- `refer/superpowers-main/skills/subagent-driven-development/SKILL.md`
- `refer/superpowers-main/RELEASE-NOTES.md`
- `refer/agent-source-code/codex-main/docs/agents_md.md`
- `refer/agent-source-code/gemini-cli-main/GEMINI.md`
- `refer/agent-source-code/claude-code-main/README.md`

### 7.2 官方公开资料

- Anthropic Claude Code Hooks  
  `https://docs.anthropic.com/en/docs/claude-code/hooks`
- Anthropic Claude Code Sub-agents  
  `https://docs.anthropic.com/en/docs/claude-code/sub-agents`
- OpenAI Codex AGENTS.md  
  `https://developers.openai.com/codex/guides/agents-md`
- Gemini CLI Docs  
  `https://geminicli.com/docs/`
- LangGraph Overview  
  `https://docs.langchain.com/oss/python/langgraph/overview`
- Temporal Docs  
  `https://docs.temporal.io/`

## 8. 建议的下一步

建议按以下顺序继续推进：

1. 先做 `controller`
2. 再做 `transition-applier`
3. 再做 runtime-native `verify/report`
4. 然后决定并行策略（先禁用还是正式落地）
5. 最后补 coverage adapter

对应 backlog 见：

- [V4 修复 Backlog（2026-03-09）](./v4-remediation-backlog-20260309.md)
