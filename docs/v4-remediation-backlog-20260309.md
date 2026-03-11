# V4 修复 Backlog（2026-03-09）

## 1. 目标

本清单用于承接《[V4 严格自我评审（2026-03-09）](./v4-self-review-20260309.md)》中的 `CONCERNS` 结论。

原则：

- 不推翻 `V4` 主体架构
- 先补执行闭环，再补增强能力
- 优先把“可运行”做实，再把“更智能”做强

## 2. 优先级总览

| ID | 优先级 | 主题 | 目标 |
| --- | --- | --- | --- |
| B1 | P0 | 统一控制器 | 让 runtime 具备正式 run / resume / verify 入口 |
| B2 | P0 | 状态推进提交器 | 把 route 决策、状态落盘、事件写入收口 |
| B3 | P1 | Runtime 原生验证报告 | 让 `ai.report.json` 由 runtime 正式生成 |
| B4 | P1 | 单活/并行策略定版 | 消除 schema 预留与 runtime 能力不一致 |
| B5 | P2 | Coverage adapter | 给多语言、多框架提供统一 coverage 证据接口 |

## 3. backlog 详情

### B1：统一控制器（P0）

**问题**

当前缺少统一执行入口，`engine-kernel` 更像内核函数，不是正式运行器。

**交付物**

- `scripts/runtime/controller.cjs`
- CLI 入口：
  - `--mode run`
  - `--mode resume`
  - `--mode verify`
- 基础命令文档

**完成标准**

- 可以从 `.ai/workflow/session.yaml` 启动一次正式 route cycle
- 可以在 `restore_pending=true` 时恢复执行
- 可以在 `review` 阶段触发 verify 流程

**建议验证**

```bash
node scripts/runtime/controller.cjs --root . --mode run
node scripts/runtime/controller.cjs --root . --mode resume
node scripts/runtime/controller.cjs --root . --mode verify
```

### B2：状态推进提交器（P0）

**问题**

当前 route 结果、状态更新、event 追加、sprint-status 同步还没有统一事务边界。

**交付物**

- `scripts/runtime/transition-applier.cjs`
- transition bundle schema
- crash/retry fixture

**完成标准**

一次 transition 至少能统一处理：

- `session.yaml`
- `task-graph.yaml`
- `sprint-status.yaml`
- `journal.jsonl`

并且在中途失败时不会留下难以解释的半提交状态。

**建议验证**

```bash
node tests/runtime/run-transition-fixtures.cjs
```

### B3：Runtime 原生验证报告（P1）

**问题**

现在 `ai.report.json` 仍偏 skill 组装，不是 runtime 原生命令。

**交付物**

- `controller --mode verify --write-report`
- report builder
- verify staging -> report merge contract

**完成标准**

- 不依赖人工整理，也不依赖 skill 直接拼装最终报告
- hook 只负责采证，runtime 负责定稿
- report 中明确区分：验证 READY 与 review verdict

**建议验证**

```bash
node scripts/runtime/controller.cjs --root . --mode verify --write-report
'{}' | node scripts/hooks/sy-stop.cjs
```

### B4：单活/并行策略定版（P1）

**问题**

规范已预留 `parallel_group`，但 runtime 仍是单活模型，容易让使用者误判能力边界。

**交付物（两选一）**

方案 A（推荐当前先做）：

- V1 明确单活执行
- 去掉或冻结对外并行承诺
- 文档与 schema 注释统一

方案 B（后续增强）：

- `parallel_group`
- `join barrier`
- 子预算分配
- 并行恢复策略

**完成标准**

- 文档、schema、runtime 三者一致
- 不再出现“规范说像支持并行，但 runtime 实际不支持”的模糊区间

**建议验证**

```bash
node tests/router/run-router-fixtures.cjs
node tests/e2e/run-engine-conformance.cjs --all
```

### B5：Coverage Adapter（P2）

**问题**

coverage policy 已存在，但不同语言、框架还没有统一证据适配层。

**交付物**

- `scripts/runtime/coverage-adapter.cjs`
- coverage evidence contract
- 至少一组 fixture（例如 Node/Vitest）

**建议字段**

- `actual`
- `required`
- `coverage_mode`
- `coverage_profile`
- `globalRegressed`
- `touchedRegionRegressed`
- `characterizationAdded`

**完成标准**

- `policy.cjs` 不再吃“随意形状”的 coverageEvidence
- `ai.report.json` 的 coverage 不再长期停留在 `n/a`

## 4. 推荐执行顺序

```text
B1 controller
-> B2 transition-applier
-> B3 runtime-native verify/report
-> B4 single-active / parallel policy finalization
-> B5 coverage adapter
```

## 5. 暂不建议现在做的事

以下事项不是当前最优先：

- 过早做复杂并行 phase/node 执行
- 过早抽象多仓库模板系统
- 过早追求“所有语言的 coverage 一步到位”
- 在没有 controller 之前继续扩充更多 persona / capability

## 6. 复审触发条件

当以下三项完成后，建议发起下一轮正式 review：

1. `controller` 已能实际驱动一次 run / resume / verify
2. transition 已有统一提交边界
3. `ai.report.json` 已能由 runtime 正式生成
