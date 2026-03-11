# V4 严格自我评审（Round 4，2026-03-09）

## 1. 本轮目标

在已有 `completion` 三段式闭环基础上，补齐最终缺的那一步：

- 当 `stop_gate_ready = true`
- 且没有可执行的 next phase / next node

runtime 不能只停留在“`hold + no executable next node`”这种半终态，
而必须进入**可审计、可恢复、可交接**的 terminal handoff。

## 2. 本轮结论

**Verdict：PASS**

本轮已经把“最终停止态”做成 machine-closed：

- router 能识别 terminal handoff 场景
- router 会输出 `human_intervention -> session`
- router 会显式发出 `session_stopped`
- verify 模式下的 transition 会把 session / phase / node 持久化到终态
- report 在 `phase.status = completed` 时仍可保持 `READY`
- controller 会把“已进入终态交接”明确暴露给人类侧状态输出

## 3. 本轮实现

### 3.1 Router：补 terminal handoff 分支

文件：`scripts/runtime/router.cjs`

新增行为：

- 当 `completion.stop_gate_ready = true` 且不存在可执行 next phase / node 时：
  - `route_verdict = hold`
  - `next_persona = human`
  - `next_capability = human_approval`
  - `recommended_next[0] = human_intervention:session`
  - 发出：`session_stopped`
- 若当前 review node 同时满足闭合条件，还会一并发出：
  - `node_completed`
  - `phase_completed`

同时修复一个隐藏问题：

- 已 `completed` 的 next phase 不再被当成可继续 `advance` 的目标

### 3.2 Transition：verify 模式不再总是 no-op

文件：`scripts/runtime/transition-applier.cjs`

之前 verify 模式对 runtime state 是只读的。
这会导致：

- `stop_gate_ready = true`
- 但 session 永远停在 `review`
- `session_stopped` 永远不落库

现在 verify 模式在 terminal handoff 场景下会：

- 完成当前 review node
- 将当前 phase 标记为 `completed`
- 将 session 置为：
  - `phase.status = completed`
  - `node.active_id = none`
  - `node.state = idle`
  - `node.owner_persona = human`
- 追加 journal 事件：`session_stopped`

### 3.3 Report：completed 终态仍可 READY

文件：`scripts/runtime/report-builder.cjs`

之前 report 只接受 `phase.status = review`。
现在扩展为：

- `review`
- `completed`

只要验证证据完整且无 blocker，report 仍为 `READY`。

### 3.4 Controller：人类可读输出补齐

文件：`scripts/runtime/controller.cjs`

本轮顺手修复了人类侧 summary 的乱码，并新增：

- `terminal_handoff_ready`
- `session_stopped`

这样人类看 controller 输出时，能直接知道：

- 只是 review ready
- 还是已经真正进入终态交接

## 4. 新增验证

### Router fixture

- `tests/router/fixtures/final-stop-gate-handoff.json`

### Transition fixture

- `tests/runtime/run-transition-fixtures.cjs`
  - `verify-terminal-handoff-completes-session`

### Controller fixture

- `tests/runtime/run-controller-fixtures.cjs`
  - `verify-stop-gate-finalizes-session`

## 5. Fresh Verification Evidence

本轮执行并通过：

- `npm run test:runtime:router`
- `npm run test:runtime:controller`
- `npm run test:runtime:report`
- `npm run test:runtime:p2`
- `npm run test:runtime:transition`
- `npm run test:runtime:repair`

## 6. 当前判断

如果现在问：

> V4 runtime 是否已经具备从 verify 闭环自然收口到 terminal handoff 的能力？

答案是：

**是。现在它不只是“验证完成”，而是“验证完成后能以 machine-readable 的方式停下来并交给人”。**
