# router.spec.yaml 中文草案

状态：人类审阅稿  
定位：用于后续翻译成 `workflow/router.spec.yaml` 的逻辑草案  
说明：本文件面向人类审核，不是最终机器规格  
现状：现行实现以 `workflow/router.spec.yaml` 与 `scripts/runtime/router.cjs` 为准，本文件已按实现收敛

## 1. 文档目标

`router.spec.yaml` 需要解决 5 个核心问题：

- 当前是否可以进入某个 `phase`
- 当前是否可以进入某个 `node`
- 当前 `node` 是否已经完成
- 当前 `phase` 是否已经完成
- 当前系统应该把 `recommended_next` 指向什么动作

它必须做到：

- 确定性：相同输入状态，得到相同路由结果
- 可恢复：会话中断后，能依据状态继续，不靠聊天记忆
- 可审计：每次路由决策都能解释依据
- 可阻断：遇到审批、预算、验证、恢复等问题时，必须停下来

## 2. Router 的职责边界

Router 负责：

- 读取 runtime 当前状态
- 识别全局阻塞条件
- 计算 phase / node 的 ready / blocked / done
- 根据 capability binding 选择下一个 persona 与 capability
- 生成结构化 `recommended_next`
- 触发应写入 journal 的路由事件

Router 不负责：

- 直接修改代码
- 直接审批危险动作
- 决定底层具体 tool 如何执行
- 判定测试门细节是否合法
- 承担完整 action DSL 的定义

一句话理解：

- Router 决定“往哪走”
- Policy 决定“能不能走”
- Hooks 决定“是否物理拦截”
- Capability 层决定“这一类动作由谁、用什么能力去做”

## 3. Router 的输入

Router 每次决策至少读取以下输入：

- `session`（包含 approvals / recovery / loop_budget 字段）
- `task_graph`
- `sprint_status`
- `validator_verdict`
- `policy_verdict`

建议输入来源：

- `session.yaml`
- `task-graph.yaml`
- `sprint-status.yaml`

现行实现暂不直接消费（保留为可扩展输入）：

- `journal` 最近事件摘要
- capability / persona binding 信息

原则：

- 以 durable state 为准
- 不以聊天上下文为准
- 不以 agent 自我感觉为准

## 4. Router 的输出

Router 每轮必须至少产出：

- `route_verdict`
- `active_phase`
- `active_node`
- `next_persona`
- `next_capability`
- `recommended_next`
- `block_reason`
- `route_basis`
- `emit_events`

建议语义：

- `route_verdict`
  - `advance`
  - `hold`
  - `block`
  - `resume`
  - `handoff`
- `next_capability`
  - 说明本轮推荐进入的能力类别，而不是具体 tool 调用
- `block_reason`
  - 为什么当前不能推进
- `route_basis`
  - 本次决策依据哪些状态字段和规则
- `emit_events`
  - 本轮应追加到 journal 的事件

## 5. Router 的总原则

### 5.1 Blocker First
先看是否有全局阻塞，再看能否推进。

### 5.2 Resume First
如存在恢复需求，先恢复，再继续计划或执行。

### 5.3 State Over Chat
只根据 runtime 状态判断，不根据聊天历史猜测。

### 5.4 Phase Before Node
先确定当前 phase，再在 phase 内选择 node。

### 5.5 Policy Before Advance
任何推进动作都必须先满足 policy 门禁。

### 5.6 Deterministic Recommended Next
`recommended_next` 必须可复算，不能依赖模糊语言。

### 5.7 Explicit Tie-Breakers
当存在多个候选 node 时，优先使用显式优先级与稳定 tie-breaker，而不是隐式“感觉最合理”。

## 6. 全局阻塞条件（Global Blockers）

Router 在尝试推进前，必须先检查以下 blocker（与现行实现一致）：

- `validator_verdict.valid=false`（包含 runtime 字段缺失与审批队列超限）
- `session.approvals.pending=true`
- `session.recovery.restore_pending=true`
- `policy_verdict.route_effect=require_human`
- 预算耗尽（nodes 或 failures 超限）
- 当前 phase 依赖未满足
- 当前 node review 失败（rework / fail）
- 当前 node 失败且无可用恢复路径

建议 blocker 优先级从高到低：

1. 状态非法 / validator fail
2. 审批阻塞（pending 或 policy require_human）
3. 恢复阻塞
4. 预算阻塞
5. review 阻塞
6. phase 依赖未满足

规则：

- 任意 blocker 命中时，Router 不得继续推进执行
- 必须输出明确 `block_reason`
- 必须生成可读且可执行的 `recommended_next`

## 7. Phase 路由规则

### 7.1 Phase 的角色
Phase 是大阶段边界，不是细粒度执行单元。

Phase 负责：

- 定义大阶段先后顺序
- 定义阶段进入条件
- 定义阶段退出门
- 定义阶段级回滚边界

### 7.2 `PhaseReady(phase)` 判定

某个 phase 可以进入 `in_progress`，当且仅当：

- 该 phase 当前状态不是 `completed`
- 该 phase 的 `depends_on` 全部已完成
- 该 phase 的 `entry_condition` 全部满足
- 没有全局 blocker
- 当前没有别的 active phase 正在执行

结论：

- V4 第一版只允许一个 active phase
- Phase 默认串行，不做并行 phase

### 7.3 `PhaseDone(phase)` 判定

某个 phase 可以进入 `completed`，当且仅当：

- 该 phase 下所有必需 node 已完成
- 没有 `failed` / `rework` 未处理 node
- `exit_gate` 已通过
- 没有待处理审批
- 没有恢复阻塞

### 7.4 Phase 状态流转建议

建议 phase 状态流转：

- `pending`
- `in_progress`
- `blocked`
- `review`
- `completed`

推荐转移关系：

- `pending -> in_progress`
- `in_progress -> blocked`
- `blocked -> in_progress`
- `in_progress -> review`
- `review -> completed`
- `review -> blocked`

### 7.5 Phase 完成后的推进

当前 phase 完成后，Router 应：

- 记录 `phase_completed`
- 选择下一个满足 `PhaseReady()` 的 phase
- 更新 `session.phase.current`
- 生成新的 `recommended_next`

说明：

- 当前 runtime 建议补一个 `phase_completed` 事件
- 否则 phase 结束只能靠状态推导，审计性不够好

## 8. Node 路由规则

### 8.1 Node 的角色
Node 是最小可执行单元。

Node 建议至少具备以下路由相关属性：

- `objective`：目标
- `capability`：能力类别
- `priority`：路由优先级
- `condition`：最小版条件表达式
- `verify`：完成验证
- `review_state`：评审状态
- `evidence_refs`：证据引用

说明：

- `capability` 用于把 node 与 persona / tool 能力解耦
- Router 通过 capability 选择“谁来做”，而不是直接决定具体工具细节
- 具体 action / input / output DSL 建议后续独立成 execution 规格，不直接塞进 router 核心规则

### 8.2 `NodeReady(node)` 判定

某个 node 可以进入 `ready` 或 `in_progress`，当且仅当：

- 所属 phase 当前为 `in_progress`
- 该 node 当前状态不是 `completed`
- `depends_on` 中所有 node 都已 `completed`
- 若存在 `condition`，则条件计算结果为 `true`
- 没有全局 blocker
- 若 `tdd_required=true`，则当前 node 的 TDD 状态允许推进
- 若 node 需要审批，则审批条件满足
- 若 node 声明了 `capability`，则 Router 能为其解析出合法的 persona / capability 绑定

### 8.3 `NodeDone(node)` 判定

某个 node 可以进入 `completed`，当且仅当：

- `verify.cmd` 已通过
- 必需 evidence 已记录
- 所需 review 已通过
- 若 `tdd_required=true`，则 `tdd_state=verified`
- `behavior_gate=pass`
- 覆盖率符合规则
- 没有待处理审批

### 8.4 Node 状态流转建议

建议 node 状态：

- `pending`
- `ready`
- `in_progress`
- `blocked`
- `review`
- `completed`
- `failed`

推荐流转关系：

- `pending -> ready`
- `ready -> in_progress`
- `in_progress -> blocked`
- `blocked -> in_progress`
- `in_progress -> review`
- `review -> completed`
- `review -> blocked`
- `in_progress -> failed`
- `failed -> blocked`
- `blocked -> ready`

补充说明：

- `failed` 应被视为错误捕获态，而不是长期停留的终态。
- node 进入 `failed` 后，Router 应先记录 `node_failed` 事件，再决定后续去向。
- 默认路径应为 `failed -> blocked`，用于等待人工介入、预算补充、审批决议或恢复动作。
- 若失败是可重试且前置条件已恢复，Router 可以将 node 从 `blocked` 重新转回 `ready`，再按正常规则进入 `in_progress`。
- 只有在策略明确允许自动重试、预算充足且不存在 blocker 时，才允许把失败后的 node 重新推进执行。
- 一个 node 失败不得让 phase 或会话永久死锁；Router 必须给出明确的下一步：人工介入、恢复、补预算或重试。

### 8.5 Ready Set 排序规则

当同一 phase 内存在多个 `NodeReady(node)=true` 的候选 node 时，Router 应采用显式排序规则：

1. `priority` 高者优先
2. 依赖链更短或更靠前的拓扑顺序优先
3. 与当前 active context 更接近的 node 优先
4. 若仍冲突，使用稳定字段（如 `id`）做最终 tie-breaker

说明：

- V1 不强制引入数值化 route score
- 原因是显式排序更容易审计、解释和回放
- 如未来确有需要，可在规则稳定后再增加 scoring 作为二级排序机制

### 8.6 Conditional Node（最小版）

V1 建议支持最小版 conditional node：

- `condition` 只能引用 durable state、policy verdict 或前序 node 结果
- 不允许执行任意脚本
- 不允许依赖聊天上下文自由解释

建议语义：

- `condition=true`：node 按正常规则参与 ready 计算
- `condition=false`：node 不进入当前 run 的 required execution set，V1 仅在 route basis 记录 bypass 语义，`node_bypassed` 事件预留

补充说明：

- 当前 runtime 尚未定义 `skipped` / `not_applicable` 状态
- 因此 V1 仅用 route basis 表达 bypass 语义
- 后续若 runtime 引入显式状态或事件，再做进一步收敛

## 9. Node 并行 / 串行规则

### 9.1 V4 第一版默认策略

- 同一 run 内默认串行执行 node
- 当前 runtime 只有单个 `active_id`
- 因此 V4 第一版不应承诺“同一 run 真并行执行多个 node”

### 9.2 `parallel_group` 的意义

当前 `parallel_group` 只建议作为：

- 并行潜力标记
- 或未来扩展的规划信息

而不是当前版本的真并行执行保证。

### 9.3 若未来启用 node 并行，必须满足

- 同属一个 phase
- 没有依赖边
- `parallel_group` 相同
- 不共享高风险可变资源
- 风险等级不是 `critical`
- 不共享同一审批对象
- adapter / runtime 支持多 active node

### 9.4 当前建议

- V4 第一版：Router 只做串行调度
- `parallel_group` 先保留，不立即激活真实并行

## 10. Persona 与 Capability 路由规则

Router 必须根据当前状态选择下一个 persona，并通过 capability binding 决定“谁最适合执行这个 node”。

建议最小 persona 集：

- `planner`
- `author`
- `spec_reviewer`
- `quality_reviewer`
- `reader`
- `auditor`
- `human`

建议 capability 作为独立层存在，例如：

- `analysis`
- `planning`
- `code_edit`
- `test_run`
- `spec_review`
- `quality_review`
- `human_approval`

### 10.1 默认路由规则

- 缺少计划结构 -> `planner` + `planning`
- node 进入实现 -> `author` + `code_edit`
- 实现完成待规格检查 -> `spec_reviewer` + `spec_review`
- 规格检查通过后 -> `quality_reviewer` + `quality_review`
- 遇到理解不清或需结构分析 -> `reader` + `analysis`
- 遇到证据核查或声明核实 -> `auditor` + `analysis`
- 审批、冲突、例外 -> `human` + `human_approval`

### 10.2 Review 链

V4 建议固定 review 顺序：

- `author`
- `spec_reviewer`
- `quality_reviewer`

未通过前，不得跳过 review 直接完成 node。

### 10.3 与 Tool Schema 的边界

- Router 应路由到 capability，而不是路由到具体 shell / patch / tool 调用
- 具体 action / tool schema 建议后续定义在独立 execution 规格中
- 这样可以保持：
  - Router = 决策层
  - Capability = 能力抽象层
  - Tool / Action = 执行层

## 11. `recommended_next` 生成规则

### 11.1 目标

`recommended_next` 不是聊天建议，而是 Router 的结构化输出。

### 11.2 生成优先级

当前实现按以下顺序生成（从高到低）：

1. `invalid_state`（validator 失败）
2. `approval_pending`
3. `restore_pending`
4. `policy_verdict.route_effect=require_human`
5. `budget_exhausted`
6. active node 继续 / 重试 / review handoff
7. phase 完成推进
8. stop gate 终止 handoff
9. ready node 启动
10. 无可执行节点的人类介入

### 11.3 推荐项 Machine Schema

建议每个推荐项至少包含：

- `type`
- `target`
- `params`
- `reason`
- `blocking_on`
- `priority`

建议语义：

- `type`
  - `resume_node`
  - `start_node`
  - `request_approval`
  - `enter_phase`
  - `human_intervention`
  - `retry_node`
- `target`
  - 目标 phase、node、approval 或人工动作
- `params`
  - 机器可消费的参数对象
- `priority`
  - `now | next | later`

示例：

```yaml
recommended_next:
  - type: request_approval
    target: approval.git_commit
    params:
      grant_scope: session
      approval_mode: manual_required
    reason: 当前 node 已完成验证，但提交操作尚未获批
    blocking_on:
      - approval.pending
    priority: now

  - type: start_node
    target: node.router_tests
    params:
      capability: test_run
    reason: 当前 phase 存在下一个 ready node
    blocking_on: []
    priority: next
```

### 11.4 生成规则

- 最多输出 3 条
- 第一条必须是当前最可执行的动作
- 若存在 blocker，第一条必须先解决 blocker
- 不允许同时输出相互冲突的建议
- `recommended_next` 必须足够结构化，以便后续 adapter 或 UI 可直接消费

## 12. 恢复与继续执行规则

### 12.1 `ResumeNeeded()` 判定

以下情况命中时，Router 必须优先进入恢复流程：

- `restore_pending=true`
- 最近事件显示中断发生在 node 未完成时
- checkpoint 存在但 session 未正常闭合
- validator 认定当前状态需恢复

### 12.2 恢复后的路由

恢复完成后，Router 应：

- 校验 checkpoint 完整性
- 恢复 `session.phase.current`
- 恢复 `session.node.active_id`
- 重新计算 blocker
- 重新生成 `recommended_next`

## 13. Budget 路由规则

Router 当前直接检查两类预算：

- 节点预算
- 失败预算

审批队列预算（`pending_count`）由 validator 归入 `invalid_state`，在全局阻塞阶段处理。

### 13.1 `BudgetAvailable()` 判定

仅当以下都成立时，才允许继续自动推进：

- `consumed_nodes < max_nodes`
- `consumed_failures < max_failures`

### 13.2 预算耗尽后的行为

预算耗尽时：

- 不得继续自治推进
- 必须输出 `block_reason=budget_exhausted`
- 必须生成面向人的下一步建议
- 应记录 `budget_exhausted` 事件

### 13.3 Node 最小 retry / timeout 字段

Router V1 不负责实现完整调度器，但应能消费最小执行韧性字段：

- `retry_policy`
  - 必填最小字段：`max_attempts`、`backoff_mode`
  - 可选字段：`initial_delay_seconds`、`max_delay_seconds`、`retry_on`
- `timeout_policy`
  - 必填最小字段：`timeout_seconds`、`on_timeout`
  - 可选字段：`grace_seconds`

最小语义：

- 若 node 超时，必须先记录 `node_timed_out` 事件
- 然后按 `on_timeout` 路由到：`fail_node | block_node | require_human`
- V1 先支持“结构化声明 + 路由后果”，不在 Router 主体内承诺完整的 backoff / deadline / SLA 编排

## 14. 事件写入规则

Router 不直接保存历史文本，但必须声明本轮应触发的 journal 事件。

建议支持：

- `phase_entered`
- `phase_completed`
- `node_started`
- `node_completed`
- `node_failed`
- `node_timed_out`
- `node_bypassed`（预留）
- `review_verdict_recorded`
- `session_resumed`
- `budget_exhausted`
- `approval_requested`
- `approval_resolved`

原则：

- 关键路由决策必须事件化
- 不允许只改状态、不写事件

## 15. Formal Route Rules（机器规则模板）

Router 最终应从“说明文档”收敛为“可执行规则对象”。

建议至少把以下规则写成机器化结构：

### 15.1 `PhaseReady(phase)`

```yaml
PhaseReady:
  requires:
    - phase.state != completed
    - depends_on.all_completed
    - entry_condition.pass
    - no_global_blocker
    - no_other_active_phase
```

### 15.2 `PhaseDone(phase)`

```yaml
PhaseDone:
  requires:
    - required_nodes.all_completed
    - no_unresolved_failed_node
    - exit_gate.pass
    - no_pending_approval
    - no_restore_pending
```

### 15.3 `NodeReady(node)`

```yaml
NodeReady:
  requires:
    - phase.state == in_progress
    - node.state not in [completed]
    - depends_on.all_completed
    - condition.pass_or_absent
    - no_global_blocker
    - tdd_state.allows_progress
    - approval.pass_or_absent
    - capability.binding_resolved
```

### 15.4 `NodeDone(node)`

```yaml
NodeDone:
  requires:
    - verify.pass
    - evidence.present
    - review.pass
    - tdd_state == verified or tdd_not_required
    - behavior_gate.pass
    - coverage.pass
    - no_pending_approval
```

### 15.5 `BudgetAvailable()`

```yaml
BudgetAvailable:
  requires:
    - consumed_nodes < max_nodes
    - consumed_failures < max_failures
```

## 16. Route Basis（路由依据）要求

每次路由决策都必须能回答：

- 读取了哪些状态字段
- 命中了哪些规则
- 为什么没选其它候选 phase / node
- 为什么当前是 `advance` / `hold` / `block` / `resume`

建议以结构化方式保留：

- `basis.session_fields`
- `basis.phase_checks`
- `basis.node_checks`
- `basis.policy_verdicts`
- `basis.blockers`
- `basis.sorting_decision`

这样后续审计和调试会简单很多。

## 17. 与 Policy 的边界

Router 可以判断“某 node 形式上 ready”，但只有在 policy 允许时才能真正推进。

例如：

- Router 认为 node 可进入 GREEN
- 但 Policy 发现 RED 非法
- 则 Router 必须把结果降级为 `block`

所以建议规则是：

- Router 先算候选
- Policy 再裁决是否允许
- 最终路由结果以 Policy 裁决后的结果为准

## 18. 与当前 runtime 的对齐说明

这份 router 草案和当前 runtime 基本对齐，但建议后续同步以下扩展：

- `journal` 事件中 `phase_completed` 已实现
- `node_bypassed` 事件仍预留（当前路由不发出）
- `task_graph.nodes` 已包含 `capability`
- `task_graph.nodes` 已包含 `priority`
- `task_graph.nodes` 已包含最小版 `condition`
- 当前 `session.node.active_id` 是单值，因此 V4 第一版 Router 应保持单 active node 串行调度

也就是说：

- phase 并行：暂不支持
- node 真并行：暂不支持
- `parallel_group`：保留为未来能力标记

## 19. 暂不纳入 Router V1 的能力

以下能力值得做，但不建议直接塞进 `router.spec.yaml v1` 主体：

- 完整 node action / input / output schema
- 高级 retry / backoff 调优策略
- 高级 timeout / deadline / SLA 编排策略
- 数值化 route score
- 多 active node 并行调度

建议将这些能力拆分到：

- `runtime.schema.yaml`
- `policy.spec.yaml`
- `workflow/persona-bindings.yaml`
- 后续可选：`workflow/capabilities.yaml`
- 后续可选：`workflow/execution.spec.yaml`

## 20. 建议你重点审核的 8 个点

建议你先拍板下面 8 项：

- 是否接受 V4 第一版 phase 串行
- 是否接受 V4 第一版 node 串行调度
- 是否接受 `parallel_group` 先只作为未来并行标记
- 是否接受 capability 层进入 Router 输入与输出
- 是否接受 node 增加 `priority`
- 是否接受最小版 `condition`
- 是否接受 `recommended_next` 升级为 machine schema
- 是否接受 `phase_completed` 与 `node_bypassed` 作为建议补充事件
