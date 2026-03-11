# V4 严格自我评审（Round 2，2026-03-09）

## 1. 评审范围

- `docs/architecture-v4.md`
- `workflow/runtime.schema.yaml`
- `workflow/router.spec.yaml`
- `workflow/policy.spec.yaml`
- `scripts/runtime/*`
- `.ai/workflow/*`
- `.ai/analysis/ai.report.json`
- `refer/agent-source-code/*`
- `refer/superpowers-main/*`
- `refer/everything-claude-code-main/*`

本轮评审遵循“证据优先”。
先看 fresh runtime，再看 refer 设计，再看官方资料。

## 2. 最终结论

**Verdict：`CONCERNS`**

一句话结论：

> `seeyue-workflows` 的 V4 主体方向是对的，machine spec 分层也已经明显成型；但 runtime 的“最终验证闭环”还没有彻底收口，所以现在更像 `almost production-grade control plane`，还不是完全闭环的 autonomous engine。

这次结论不是 `REWORK`，因为：

- source of truth 已经从聊天记忆迁到 `workflow/*.yaml` + `.ai/workflow/*`
- Router / Policy / Runtime Schema 三层分工成立
- TDD / approval / retry / timeout / review 这些关键语义已经进入 machine spec
- hooks 已经承担物理拦截职责

但这次也不能给 `PASS`，因为 runtime verify 还有真实缺口，且已经被 fresh evidence 证明。

## 3. Fresh Evidence

### 3.1 当前 verify 结果

执行：

```bash
node scripts/runtime/controller.cjs --root . --mode verify --write-report --json
```

当前结果要点：

- `validator_verdict.valid = true`
- `route_verdict = hold`
- `policy_verdict.completion.node_complete_ready = false`
- `policy_verdict.completion.reasons = ["node_completion_incomplete"]`
- `verification.report_overall = NOT_READY`

结论：

- runtime 已经不再卡在 `invalid_state`
- 但 verify 仍然无法把会话推进到 `review_ready = true`

### 3.2 已修复的真实运行态问题

本轮先发现并修复了一个真实 runtime 漂移：

- `.ai/workflow/task-graph.yaml` 里仍残留非空 `parallel_group`
- 但 `workflow/router.spec.yaml` 已经明确：
  - `phase_routing.execution_mode = single_active_phase`
  - `node_routing.execution_mode = single_active_node`
  - `parallel_phases_supported = false`
  - `parallel_nodes_supported = false`

因此原来会导致：

- `parallel_group_not_supported_v1`
- `block_reason = invalid_state`

本轮已将 `.ai/workflow/task-graph.yaml` 中 4 处旧残留并行组改为 `null`，现在 verify 已不再因 invalid runtime state 被阻塞。

### 3.3 当前 report 状态

当前 `.ai/analysis/ai.report.json` 已刷新，但仍为：

- `overall = NOT_READY`
- `verification.tests[*].status = n/a`
- `verification.coverage.actual = n/a`

同时：

- `.ai/analysis/verify-staging.json` 当前不存在

这说明 report builder 已 runtime-native，
但 verify evidence 采集链还没有真正 runtime-native。

## 4. 核心判断

## 4.1 做对了什么

### A. 设计方向已经对齐主流 agent 控制面

从 refer 和官方资料看，V4 现在最对的三件事是：

1. **state over chat**
   - 这和 Gemini CLI 的 checkpoint / plan / durable session 思路一致
   - 也和 Temporal / workflow engine 的 durable execution 思路一致

2. **instruction / policy / hook 分层**
   - 这和 Codex 的 `AGENTS.md` 分层、审批/沙箱分层一致
   - 也和 Claude Code 的 hooks + subagents 分层一致

3. **task-isolated review chain**
   - 这和 Superpowers 的 fresh subagent per task + spec review before quality review 高度一致
   - 也和 Everything-Claude-Code 的 research-first / TDD-first / review-first 风格一致

### B. Router 规格已经达到可执行级别

`workflow/router.spec.yaml` 现在已经不是“说明文档”，而是可驱动实现的 machine contract。

它已经具备：

- global blockers 优先级
- phase / node 双层状态机
- `recommended_next` machine schema
- failure / retry / timeout 出口
- capability / persona / priority / condition 支撑点

这比大量“只有 prompt，没有 runtime”的 workflow 项目成熟得多。

### C. Policy 规格已经能做物理测试门

`workflow/policy.spec.yaml` 的成熟度也很高，尤其是：

- RED 合法失败类型与非法失败类型分离
- behavior gate 和 coverage gate 分离
- legacy code 的 patch coverage / delta coverage 语义
- notify-only 只在低风险、可审计、小改动下成立

这一层已经不只是“倡议 TDD”，而是开始具备“阻止错误执行”的能力。

## 4.2 真正还没做完什么

### A. verify 仍是“报告生成已下沉，证据采集未下沉”

这是本轮评审最重要的发现。

现在的事实是：

- `scripts/runtime/report-builder.cjs` 已经可以由 runtime 原生生成 `ai.report.json`
- 但 `scripts/runtime/controller.cjs --mode verify` 传给 `runEngineKernel` 的 `actionContext` 仍是空对象
- `scripts/runtime/policy.cjs` 的 `buildCompletionVerdict()` 需要 `actionContext.verifyEvidence?.passed === true`
- 由于 verify 证据没有注入，`node_complete_ready` 永远是 `false`

这不是文档问题，而是实际运行链路问题。

也就是说：

> 现在的 runtime 已经能“写报告”，但还不能稳定地“自己收集足够的验证证据，再据此判定 READY”。

这会导致 V4 在最后一公里上仍然依赖 hook / 外层 skill / 旧报告残留，而不是完全由 runtime 驱动。

### B. report overall 的 READY 条件仍依赖 staging / 旧证据

从 `scripts/runtime/report-builder.cjs` 当前逻辑看：

- 它优先读取 `.ai/analysis/verify-staging.json`
- 没有 staging 时，退回到现有 `ai.report.json`
- 当前仓库里 staging 文件不存在
- 现有 report 中 tests/build/typecheck/lint 多数仍是 `n/a`

因此即使所有 phase/node 都完成，`overall` 仍可能留在 `NOT_READY`。

这说明：

- `report-builder` 解决了“报告输出归 runtime”
- 但还没解决“验证事实从哪里来、何时采集、由谁采集、如何回灌 policy”

### C. 单活策略已经定版，但 runtime state 迁移还不够硬

这次 `parallel_group` 漂移就是证据。

说明当前系统虽然已经把 V1 单活策略写进了：

- `workflow/router.spec.yaml`
- `workflow/runtime.schema.yaml`
- `scripts/runtime/spec-validator.cjs`
- `scripts/runtime/engine-kernel.cjs`

但老状态仍可能残留在 `.ai/workflow/task-graph.yaml` 中。

这类问题如果放到真实长会话环境，会直接表现为：

- session 无法 resume
- controller verify 阻塞
- 用户看见“明明都做完了，为什么还卡着”

所以 V4 需要补一层 **runtime state migration / repair**，而不是只靠 validator 报错。

## 5. 对照 refer / 官方资料后的评审结论

### 5.1 对照 Claude Code

Claude Code 官方文档强调两点：

- hooks 是确定性拦截层，不该依赖模型“想起来再做”
- subagents 是隔离上下文、限制工具、降低污染的执行单元

V4 已经吸收了这两点，但当前 verify 链仍偏“hook 帮忙记录，runtime 末端汇总”。

这意味着：

- **拦截边界对了**
- **最终控制面还没完全回收**

### 5.2 对照 Codex

Codex 的公开资料很强调：

- approval policy 和 sandbox mode 是两层不同控制
- `AGENTS.md` 是分层叠加的 project instruction source

V4 这部分方向是对的，甚至已经比很多 workflow 项目更系统。

但 Codex 风格还有一个隐含要求：

> 最终执行状态必须和用户看到的控制状态一致。

现在 V4 的问题就在这里：

- task graph 完成了
- report 却还没 READY
- controller verify 也无法把 completion 合上

所以不是架构方向错，而是“状态一致性闭环”还差一步。

### 5.3 对照 Gemini CLI

Gemini CLI 的 plan mode、checkpointing、trusted folders 都在强调：

- durable state
- approval boundary
- read-only planning
- 可恢复执行

V4 已经在 schema 层和 runtime store 层吸收了这些。

但 Gemini 在 checkpointing 上有一个很重要的启发：

> 只要是会改变状态的关键动作，系统就必须明确知道“之前是什么、之后是什么、如何恢复”。

V4 当前对 verify evidence 的处理，还没有完全达到这个粒度。

### 5.4 对照 Superpowers / Everything-Claude-Code

Superpowers 的强项不只是技能多，而是：

- plan 足够细
- task context 足够小
- 每个 task 都有 review
- TDD 不是口号，而是执行顺序

Everything-Claude-Code 的强项则是：

- research-first
- development workflow 明确
- `tdd-guide` / `code-reviewer` 角色边界清晰

V4 已经学到了这些“分工思想”，
但 runtime verify 还没把“测试完成 -> 证据记录 -> completion gate -> report READY”串成一条真正闭环的链。

## 6. 本轮建议优先级

### P0：补 runtime verify evidence 回灌

必须优先做。

目标不是“再写一个报告脚本”，而是：

- `controller --mode verify` 能拿到 fresh verification evidence
- evidence 能喂给 `policy.cjs`
- `buildCompletionVerdict()` 能在证据充分时得到 `node_complete_ready = true`
- `report-builder` 能据同一份证据生成 `READY`

一句话：**同一套证据同时服务 policy 和 report。**

### P1：补 runtime state repair / migration

建议新增一层 repair 能力，例如：

- 扫描旧 `parallel_group`
- 修复旧字段 / 无效字段
- 输出 repair event 到 journal

避免以后再出现“spec 已更新，但旧 runtime state 还带着旧语义”的问题。

### P2：把 coverage 从“有 adapter”推进到“仓库真实可产出”

当前 `coverage-adapter` 已完成，方向没问题。

但当前仓库根目录没有真实 coverage 产物，所以 report 还是 `n/a`。

建议后续补：

- 至少一条真实 coverage 生成路径
- 覆盖到当前仓库主测试链
- 让 `ai.report.json` 的 coverage 不再长期停在 `n/a`

## 7. 本轮变更

本轮实际已做一项修复：

- 修改 `.ai/workflow/task-graph.yaml`
- 将 4 处非空 `parallel_group` 残留改为 `null`

修复效果：

- `parallel_group_not_supported_v1` 消失
- `validator_verdict.valid` 变为 `true`
- verify 不再因 `invalid_state` 被阻塞

## 8. 结论归纳

如果站在“是否继续沿 V4 推进”的角度，我的结论是：

> **继续沿 V4 推进，不推翻；但接下来必须优先补 runtime verify 闭环，而不是继续扩展更多周边能力。**

因为现在真正卡住 V4 的，不是 router，不是 policy，也不是 hooks，
而是最后这条链还没有彻底打通：

`fresh verification evidence -> policy completion -> report READY -> human review ready`

这就是本轮最核心的审查结论。

## 9. 参考资料

### 本地 refer

- `refer/agent-source-code/claude-code-main/README.md`
- `refer/agent-source-code/codex-main/AGENTS.md`
- `refer/agent-source-code/codex-main/docs/config.md`
- `refer/agent-source-code/gemini-cli-main/docs/cli/checkpointing.md`
- `refer/agent-source-code/gemini-cli-main/docs/cli/plan-mode.md`
- `refer/agent-source-code/gemini-cli-main/docs/cli/trusted-folders.md`
- `refer/superpowers-main/skills/subagent-driven-development/SKILL.md`
- `refer/superpowers-main/skills/test-driven-development/SKILL.md`
- `refer/everything-claude-code-main/rules/common/development-workflow.md`
- `refer/everything-claude-code-main/rules/common/agents.md`
- `refer/everything-claude-code-main/skills/tdd-workflow/SKILL.md`
- `refer/everything-claude-code-main/agents/tdd-guide.md`

### 官方资料

- Claude Code Hooks  
  https://code.claude.com/docs/en/hooks
- Claude Code Subagents  
  https://code.claude.com/docs/en/sub-agents
- Codex Agent approvals & security  
  https://developers.openai.com/codex/agent-approvals-security
- Gemini CLI Checkpointing  
  https://google-gemini.github.io/gemini-cli/docs/cli/checkpointing.html

