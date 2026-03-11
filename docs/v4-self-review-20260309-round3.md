# V4 严格自我评审（Round 3，2026-03-09）

## 1. 本轮目标

补齐 runtime 最后一个关键闭环：

`fresh verification evidence -> policy completion -> router progression -> runtime transition -> report READY`

本轮关注点不再是扩功能，而是把已有 V4 语义真正落到运行时。

## 2. 本轮结论

**Verdict：PASS（针对本轮整改范围）**

原因：

- `policy` 已能正确给出 `node_complete_ready / phase_complete_ready / stop_gate_ready`
- `router` 不再把 `phase.status=review` 当作可直接前进的宽松捷径
- `router` 会在 `active node = in_progress` 时继续当前节点，而不是错误并发启动下一个节点
- `router` 会在 `review` 已通过但 `completion` 未闭合时停住当前节点，不再提前推进
- `transition` 已能在 review 节点闭合后真实落库 `node_completed`
- 真实仓库 `verify` 已得到：
  - `validator_verdict.valid = true`
  - `policy_verdict.completion.node_complete_ready = true`
  - `policy_verdict.completion.phase_complete_ready = true`
  - `policy_verdict.completion.stop_gate_ready = true`
  - `.ai/analysis/ai.report.json.overall = READY`

## 3. 本轮修复点

### 3.1 Policy：补齐 completion 末态语义

文件：`scripts/runtime/policy.cjs`

新增能力：

- 计算 approval queue 是否超预算
- 计算 loop budget 是否耗尽
- 校验 runtime 必需字段是否完整
- 校验 phase exit gate 是否通过
- 校验 review evidence 是否满足 stop gate 最小要求
- 基于 phase node 集合计算 `phase_complete_ready`
- 基于后续 phase 是否还有未完成工作计算 `stop_gate_ready`

现在 `completion` 不再只有 `node_complete_ready`，而是形成完整三段式语义：

- `node_complete_ready`
- `phase_complete_ready`
- `stop_gate_ready`

### 3.2 Router：收紧推进条件

文件：`scripts/runtime/router.cjs`

修复点：

- 新增 `in_progress -> resume current node` 保护，避免单活节点模型被破坏
- 新增 `review accepted but completion incomplete -> hold current node` 保护
- 去掉 `phase.status == review` 就可直接推进 phase 的宽松逻辑
- 仅在 phase 真正满足 completion 条件时才允许 `enter_phase`
- 当 review 节点已闭合并准备切换时，显式发出 `node_completed`

### 3.3 Transition：把语义真正落库

文件：`scripts/runtime/transition-applier.cjs`

修复点：

- 当当前 active review node 已满足 `node_complete_ready` 时，先把该节点写成 `completed`
- 再进入 `start_node` 或 `enter_phase`
- 修正 journal 事件归属，确保 `node_completed` 记在旧节点上，不会误记到新节点

## 4. 新增验证用例

### Policy Fixtures

- `tests/policy/fixtures/phase-gate-ready.json`
- `tests/policy/fixtures/stop-gate-ready.json`

### Router Fixtures

- `tests/router/fixtures/review-completion-incomplete-holds-node.json`
- `tests/router/fixtures/in-progress-node-resumes.json`

### Transition Fixtures

- `tests/runtime/run-transition-fixtures.cjs`
  - 新增 `review-complete-finalizes-node-before-starting-next`

## 5. Fresh Verification Evidence

本轮执行并通过：

- `npm run test:runtime:policy`
- `npm run test:runtime:router`
- `npm run test:runtime:controller`
- `npm run test:runtime:transition`
- `npm run test:runtime:p2`
- `npm run test:runtime:report`
- `npm run test:runtime:repair`
- `node scripts/runtime/controller.cjs --root . --mode verify --repair-state --write-report --json`

关键实证结果：

- 真实仓库 verify 输出中，`completion` 三个 gate 全为 `true`
- `review_ready = true`
- `.ai/analysis/ai.report.json` 为 `READY`

## 6. 仍然保留的边界

本轮没有扩展以下内容，属于后续优化，而不是当前闭环阻塞项：

- 更细粒度的 review evidence freshness 规则
- 更强的 phase exit gate 专用证据模型
- 更完整的 session stop / terminal handoff 语义
- 并行 phase / parallel group 的正式调度实现

## 7. 当前判断

如果问题是：

> V4 runtime 这一轮是否已经从“接近可用”进入“completion 语义闭合、可继续向后推进”？

我的判断是：

**是。当前可以继续推进下一轮实现，不需要再卡在 runtime completion 基础闭环上。**
