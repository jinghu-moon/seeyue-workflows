# Hook 瘦身重构计划（Hook Slim Plan）

> 来源：`docs/MCP/08-implementation-plan.md` §7.5 第 2 条
> 编写日期：2026-03-17
> 完成日期：2026-03-17
> 前置条件：`10-three-layer-protocol.md` 已确认
> **状态：✅ 全部 Phase 完成**

---

## 1. 目标

将 `hook-client.cjs` 从「混合职责」重构为「纯 verdict 决策层」：

| 当前状态 | 目标状态 |
|---|---|
| 决策 + IO + 状态读取 + 证据记录 全部混合 | 仅做 verdict 决策，IO/记录全部委托 MCP |
| 直接读写 `session.yaml` / `journal.jsonl` | 通过 MCP `sy_*` 工具间接操作 |
| Checkpoint 逻辑内嵌 hook 脚本 | Checkpoint 由 `sy_create_checkpoint` 负责 |

---

## 2. 当前 hook-client.cjs 职责审计

| 职责 | 当前位置 | 目标位置 | 优先级 |
|---|---|---|---|
| command class 分类 | hook-client.cjs | **保留**（纯计算，无 IO）| - |
| verdict 决策（allow/block/notify）| hook-client.cjs | **保留** | - |
| file class 检查 | hook-client.cjs | **保留**（读 yaml 配置是可接受的 IO）| - |
| session.yaml 读取 | hook-client.cjs | 迁移至 `sy_pretool_bash` / `sy_pretool_write` | P1 |
| journal.jsonl 写入 | hook-client.cjs | 迁移至 `sy_posttool_write` | P1 |
| Checkpoint 创建 | hook-client.cjs | 迁移至 `sy_create_checkpoint` | P1 |
| loop budget 检查 | 部分在 hook | 标准化至 `sy_pretool_bash` + `sy_advance_node` | P2 |
| TDD 状态机推进 | 分散在多处 | 统一至 `sy_advance_node` | P2 |

---

## 3. 重构分阶段计划

### Phase 1：证据链迁移（低风险）

**目标**：journal 写入全部经由 `sy_posttool_write` MCP 工具。

**步骤**：
1. 确认 `sy_posttool_write` 接受完整证据链字段（见 `10-three-layer-protocol.md` §8）
2. 在 `hook-client.cjs` 的 `PostToolUse` 路径中，将直接 `fs.appendFileSync(journal)` 替换为调用 MCP `sy_posttool_write`
3. 删除 hook 中的 journal 直写逻辑
4. 验证：运行 QA `P1/sy_posttool_write` 测试通过

**风险**：低（MCP 工具已实现并通过测试）

### Phase 2：Checkpoint 迁移（中风险）

**目标**：Checkpoint 创建逻辑迁移至 `sy_create_checkpoint`。

**步骤**：
1. 审计 hook-client.cjs 中所有 `checkpoint` 相关逻辑
2. 确认 `sy_create_checkpoint` 的 `files` 参数覆盖现有快照需求
3. 替换 hook 中的 checkpoint 直写为 MCP 调用
4. 验证：`sy_create_checkpoint` + `diff_since_checkpoint` 配对测试

**风险**：中（需确认 SQLite WAL 并发安全性）

### Phase 3：Loop Budget 标准化（中风险）

**目标**：loop budget 六项指标检查统一在 `sy_pretool_bash` 和 `sy_advance_node` 中执行。

**步骤**：
1. 确认 `session.yaml` 中 `loop_budget` 字段完整（九字段，见 `workflow/state.rs`）
2. 在 `sy_pretool_bash` 中添加 budget 检查逻辑（参考 `09-p3-implementation-plan.md` §2.2）
3. 在 `sy_advance_node` 中添加 budget 检查逻辑
4. hook-client.cjs 删除冗余的 budget 检查分支
5. 验证：budget 超限场景返回正确的 `block` + `budget_exceeded` 响应

**风险**：中（需测试超限边界条件）

### Phase 4：session.yaml 读取迁移（高风险）

**目标**：hook 不再直接读取 session.yaml，改为从 MCP verdict 响应中获取所需状态。

**步骤**：
1. 识别 hook 中读取 session.yaml 的所有路径
2. 确认所需状态字段均已在 MCP verdict 响应中返回
3. 逐路径替换，保持 verdict 行为不变
4. 验证：全量 QA 测试通过，hook 行为与重构前一致

**风险**：高（改变 hook 信息来源，需充分测试）

---

## 4. 验收标准

| 验收项 | 方法 |
|---|---|
| hook-client.cjs 无直接 `fs.appendFileSync(journal)` | grep 检查 |
| hook-client.cjs 无直接 checkpoint 写入 | grep 检查 |
| loop budget 六项指标均在 MCP 工具中检查 | 单元测试 |
| QA 全量测试 42/42 通过 | `python tests/sandbox/qa_full.py` |
| hook verdict 行为与重构前一致 | before/after 对比测试 |

---

## 5. 回滚策略

每个 Phase 完成后创建 `sy_create_checkpoint` 快照，任意 Phase 失败时：

1. 调用 `rewind` 回滚 MCP 层变更
2. 用 git 还原 hook-client.cjs 到上一个 Phase 完成点
3. 重新运行 QA 确认恢复成功

---

## 6. 依赖关系

```
10-three-layer-protocol.md（证据链格式定义）
  ↓
Phase 1（证据链迁移）
  ↓
Phase 2（Checkpoint 迁移）
  ↓
Phase 3（Loop Budget 标准化）← 同时依赖 workflow/state.rs LoopBudget 九字段
  ↓
Phase 4（session.yaml 读取迁移）← 最高风险，最后执行
```
