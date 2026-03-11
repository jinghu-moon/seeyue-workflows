# V4 严格自我评审（Round 5，2026-03-09）

## 1. 本轮目标

补齐 V4 runtime 在 terminal handoff 之后的下一步能力：

- 旧 run 已经能安全停下
- 现在还必须能安全开启下一轮 run
- 过程中不能覆盖旧证据，不能把 active 状态和 archive 状态混在一起

换句话说，本轮要验证的是：

> runtime 是否已经具备“收口一轮、归档一轮、再启动下一轮”的完整闭环。

## 2. 本轮结论

**Verdict：PASS**

当前实现已经具备受控的 new-run bootstrap 能力：

- 只允许从干净终态启动下一轮
- 启动前会先归档上一轮 active run
- active runtime 区域会被明确清理并重建
- 新 run 会重新生成 `session.yaml`、`task-graph.yaml`、`sprint-status.yaml`、`ledger.md`
- 新 run 会写入 `session_started` 与 `phase_entered`
- 如果图重置后没有 ready node，不会擅自推进，而是回到 `human_intervention`

这使得 V4 runtime 不再只是“能停下来”，而是“能有边界地进入下一轮”。

## 3. 本轮实现

### 3.1 新增 bootstrap 入口

文件：`scripts/runtime/bootstrap-run.cjs`

新增能力：

- 校验当前 active run 是否满足 bootstrap 前提
- 拒绝对未终态 run 直接做 archive / reset
- 支持 `--task-id`、`--task-title`、`--task-mode`、`--graph`、`--engine-kind`、`--json`

### 3.2 归档策略

归档目录：`.ai/archive/<old_run_id>/`

至少归档以下内容：

- `workflow/session.yaml`
- `workflow/task-graph.yaml`
- `workflow/sprint-status.yaml`
- `workflow/journal.jsonl`
- `workflow/ledger.md`
- `workflow/capsules/`
- `workflow/checkpoints/`
- `analysis/ai.report.json`
- `analysis/verify-staging.json`
- `analysis/coverage-staging.json`
- `meta/manifest.json`

其中 `manifest.json` 会记录：

- `archived_at`
- `archived_run_id`
- `archived_paths`

### 3.3 Active runtime 重建

bootstrap 成功后，active 区域会被重新初始化：

- 清理旧的 active runtime 文件
- 重新创建 `.ai/workflow/*`
- 重置 task graph 中的 phase / node 状态
- 为新 run 生成新的 `recommended_next`

重置原则：

- 首 phase 进入 `in_progress`
- 首 phase 中无依赖 node 进入 `ready`
- 其他 node 保持 `pending`
- `tdd_required = false` 的 node 会落为 `not_applicable`
- 其余 node 从 `red_pending` 开始

### 3.4 与 terminal handoff 的关系

本轮不是替代 Round 4，而是补全 Round 4。

Round 4 解决的是：

- verify 完成后，session 如何安全停下

Round 5 解决的是：

- session 已经安全停下之后，如何安全开启下一轮

二者组合后，形成了：

- `verify -> terminal handoff -> archive -> bootstrap -> next run`

这个完整闭环。

## 4. 新增验证

### Bootstrap fixture

- `tests/runtime/run-bootstrap-fixtures.cjs`
  - `bootstrap-archives-stopped-run-and-initializes-new-run`
  - `bootstrap-refuses-active-run`

## 5. Fresh Verification Evidence

本轮执行并通过：

- `npm run test:runtime:bootstrap`
- `npm run test:runtime:store`
- `npm run test:runtime:controller`
- `npm run test:runtime:specs`

## 6. 当前判断

如果现在问：

> V4 runtime 是否已经具备“上一轮收口后，安全切换到下一轮”的基础能力？

答案是：

**是。**

但边界也要说清楚：

- 它目前只支持“从干净终态开启下一轮”
- 它不是“强制覆盖当前运行态”的命令
- 对真实仓库执行 `runtime:new-run` 之前，仍然应该先人工确认当前 run 确实已经完成交接
