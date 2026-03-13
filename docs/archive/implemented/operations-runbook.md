# 运行手册

## 目标

这份手册面向维护者，说明日常巡检、恢复、审计、发布时的最小操作顺序。

1. 先验证事实源
2. 再验证运行态
3. 再处理异常与恢复
4. 最后进入同步与发布

## 日常巡检

### 最小巡检命令

- [`../scripts/runtime/validate-specs.cjs`](../../../scripts/runtime/validate-specs.cjs)
- [`../tests/hooks/sy-hooks-smoke.cjs`](../../../tests/hooks/sy-hooks-smoke.cjs)
- [`../tests/runtime/run-engine-kernel.cjs`](../../../tests/runtime/run-engine-kernel.cjs)
- [`../tests/runtime/run-transition-fixtures.cjs`](../../../tests/runtime/run-transition-fixtures.cjs)
- [`../tests/runtime/run-coverage-adapter-fixtures.cjs`](../../../tests/runtime/run-coverage-adapter-fixtures.cjs)
- [`../tests/runtime/run-report-builder-fixtures.cjs`](../../../tests/runtime/run-report-builder-fixtures.cjs)
- [`../tests/output/run-output-template-fixtures.cjs`](../../../tests/output/run-output-template-fixtures.cjs)
- [`../tests/output/run-output-log-fixtures.cjs`](../../../tests/output/run-output-log-fixtures.cjs)
- [`../tests/output/run-output-contract-fixtures.cjs`](../../../tests/output/run-output-contract-fixtures.cjs)
- [`../tests/runtime/run-context-fixtures.cjs`](../../../tests/runtime/run-context-fixtures.cjs)
- [`../tests/runtime/run-recovery-fixtures.cjs`](../../../tests/runtime/run-recovery-fixtures.cjs)
- [`../tests/runtime/run-bootstrap-fixtures.cjs`](../../../tests/runtime/run-bootstrap-fixtures.cjs)
- [`../tests/e2e/run-engine-conformance.cjs`](../../../tests/e2e/run-engine-conformance.cjs)

建议顺序：

```bash
node scripts/runtime/validate-specs.cjs --all
npm run test:hooks:smoke
npm run test:runtime:p2
npm run test:runtime:transition
npm run test:runtime:coverage
npm run test:runtime:report
node tests/output/run-output-template-fixtures.cjs
node tests/output/run-output-log-fixtures.cjs
node tests/output/run-output-contract-fixtures.cjs
npm run test:runtime:context
npm run test:runtime:recovery
npm run test:runtime:bootstrap
npm run test:e2e:engine-conformance
node tests/e2e/run-doc-link-check.cjs
```

### Runtime 正式入口

从本次版本开始，runtime 提供最小可用的正式入口：

```bash
npm run runtime:run
npm run runtime:resume
npm run runtime:verify
npm run runtime:repair
npm run runtime:coverage:write
npm run runtime:verify:write-report
npm run runtime:new-run -- --task-id "<id>" --task-title "<title>" --task-mode "<mode>"
```

含义：

- `runtime:run`：基于当前 `.ai/workflow/*` 执行一次正式 route cycle，并同步 `sprint-status.yaml`
- `runtime:resume`：当当前 run 需要恢复时，重新进入 event loop，并把 `recommended_next` 重新固化到运行态
- `runtime:verify`：在 `phase.status=review` 时读取当前 runtime 状态与 `ai.report.json`，输出是否具备进入人工评审的条件
- `runtime:repair`：扫描当前 `.ai/workflow/*` 中可安全自动修复的旧运行态漂移，并写回运行态与审计日志
- `runtime:coverage:write`：自动发现常见 coverage 输出并写入 `.ai/analysis/coverage-staging.json`
- `runtime:verify:write-report`：由 runtime 原生生成或刷新 `.ai/analysis/ai.report.json`，再给出 `READY / NOT_READY`
- `runtime:new-run`：仅在旧 run 已进入干净终态时使用；先归档旧 run，再清空 active 运行态，并按图模板 bootstrap 一个新 run

当前边界：

- 这是 `B1 controller` 的最小落地版本
- 统一 transition 提交器、runtime-native report 写入、并行策略仍在后续 backlog 中

### 新一轮 run 启动（archive + bootstrap）

仅在以下条件全部满足时使用 `runtime:new-run`：

- 当前没有 active run；或当前 run 已进入干净终态交接
- `phase.status = completed`
- `node.active_id = none`
- `approvals.pending_count = 0`
- `recovery.restore_pending = false`

命令示例：

```bash
npm run runtime:new-run -- --task-id "P4" --task-title "Start next verified run" --task-mode "auto"
npm run runtime:new-run -- --task-id "P4" --task-title "Start next verified run" --task-mode "normal" --graph "./workflow/task-graph.yaml" --engine-kind "codex"
```

执行效果：

- 旧 run 会先归档到 `.ai/archive/<old_run_id>/`
- 归档内容至少包含 `session.yaml`、`task-graph.yaml`、`sprint-status.yaml`、`journal.jsonl`、`output.log`、`ledger.md`、`capsules/`、`checkpoints/` 与关键 `analysis` 产物
- active `.ai/workflow/*` 会被重建，active `analysis` 暂存产物会被清理
- graph 会按模板重置，首个可执行 node 会被重新标记为 `ready`
- 新 run 会写入 `session_started`、`phase_entered`，并生成新的 `recommended_next`

拒绝条件：

- 当前 run 仍然可继续执行
- 仍有 pending approval
- 仍处于 `restore_pending = true`
- 当前 active run 还没有进入干净终态

不要对仍在执行的 `.ai/workflow/*` 直接运行此命令。它的语义是“归档上一轮，再开启下一轮”，不是“强行覆盖当前运行态”。

## 运行态恢复

### 恢复入口

恢复时优先检查：

- `.ai/workflow/session.yaml`
- `.ai/workflow/sprint-status.yaml`
- `.ai/workflow/journal.jsonl`
- `.ai/workflow/checkpoints/`
- `.ai/workflow/capsules/`

重点字段：

- `session.recovery.restore_pending`
- `session.recovery.restore_reason`
- `session.recovery.last_checkpoint_id`
- `sprint-status.recommended_next`

### Gemini 恢复桥

Gemini CLI 可通过 [`../scripts/runtime/recovery-bridge.cjs`](../../../scripts/runtime/recovery-bridge.cjs) 回填 checkpoint。

命令：

```bash
node scripts/runtime/recovery-bridge.cjs --root "." --gemini-checkpoint "<checkpoint.json>"
```

恢复成功后，至少确认：

- `checkpoint_restored`
- `session_resumed`
- `recommended_next` 指向合法 phase / node

### 上下文恢复

使用 [`../scripts/runtime/context-manager.cjs`](../../../scripts/runtime/context-manager.cjs) 管理 capsule 与 handoff：

- 生成 capsule
- 更新 `resume_frontier`
- 生成 review handoff capsule

对应回归：

```bash
node tests/runtime/run-context-fixtures.cjs
```

## 审计与证据

### 关键审计文件

最少检查以下产物：

1. `journal.jsonl` 是否连续记录关键事件
2. `output.log` 是否包含关键输出模板与变量
3. `checkpoints/` 是否存在可恢复快照
4. `capsules/` 与 handoff 是否能支持 persona 切换
5. `ledger.md` 是否仍与当前规则一致

相关实现：

- [journal runtime](../../../scripts/runtime/journal.cjs)
- [checkpoint runtime](../../../scripts/runtime/checkpoints.cjs)
- [context manager](../../../scripts/runtime/context-manager.cjs)
- [engine kernel](../../../scripts/runtime/engine-kernel.cjs)

### 关键拦截点

日常审计时，重点核对以下拦截器：

- [`../scripts/hooks/sy-pretool-write.cjs`](../../../scripts/hooks/sy-pretool-write.cjs)
- [`../scripts/hooks/sy-pretool-bash.cjs`](../../../scripts/hooks/sy-pretool-bash.cjs)
- [`../scripts/hooks/sy-stop.cjs`](../../../scripts/hooks/sy-stop.cjs)
- [`../scripts/runtime/policy.cjs`](../../../scripts/runtime/policy.cjs)

## 变更与发布

发生规范或 runtime 变更时，建议按以下顺序推进：

1. 修改 `workflow/*.yaml`
2. 运行 [`../scripts/runtime/validate-specs.cjs`](../../../scripts/runtime/validate-specs.cjs)
3. 更新 `scripts/runtime/*`
4. 更新 `scripts/hooks/*` 与 `scripts/adapters/*`
5. 跑回归
6. 同步 [`./source-of-truth.md`](./source-of-truth.md)、[`./adoption-guide.md`](./adoption-guide.md)、[`./release-checklist.md`](./release-checklist.md)、[`./versioning-policy.md`](./versioning-policy.md)

发布前还要核对：

- vendor 输出没有反向覆盖事实源
- 版本策略与同步边界没有漂移
- engine conformance 结果保持通过

## 常见故障

### 1. `restore_pending=true` 但无法恢复

优先检查：

- `last_checkpoint_id` 是否存在
- `journal.jsonl` 中最后一个 `node_started` 是否缺少 terminal event
- `recommended_next` 是否仍可路由

参考回归：

- [`../tests/runtime/run-recovery-fixtures.cjs`](../../../tests/runtime/run-recovery-fixtures.cjs)
- [`../tests/runtime/run-context-fixtures.cjs`](../../../tests/runtime/run-context-fixtures.cjs)

### 2. TDD 红门误拦截

优先检查：

- [`../workflow/policy.spec.yaml`](../../../workflow/policy.spec.yaml)
- [`../scripts/hooks/sy-pretool-write.cjs`](../../../scripts/hooks/sy-pretool-write.cjs)
- `journal.jsonl` 中是否已经记录 `red_recorded`

### 3. 多引擎输出不一致

优先检查：

- [`../tests/e2e/run-engine-conformance.cjs`](../../../tests/e2e/run-engine-conformance.cjs)
- [`../tests/adapters/run-adapter-snapshots.cjs`](../../../tests/adapters/run-adapter-snapshots.cjs)
- [版本化策略](./versioning-policy.md)

如果 conformance 失败，先回到 source of truth 与 adapter compiler，再重新生成 vendor 输出。

