# P2-N3 / P2-N5 REWORK 修复总结

> Phase: P2 Interaction System
> Nodes: P2-N3 (legacy projection schema 对齐) + P2-N5 (elicitation-first orchestration)
> 审核轮次: 第四轮 REWORK → 第五轮 REWORK → 修复完成

---

## P2-N3 修复：ask_user / input_request schema 对齐

### 问题根因

`project_ask_as_interaction` 和 `project_input_as_interaction` 写入了
`workflow/interaction.schema.yaml` 不允许的字段值，测试也断言了这些错误值，
导致 gate 绿色但 canonical 文件无效。

### 修复内容

**`seeyue-mcp/src/tools/ask_user.rs`** — `project_ask_as_interaction`:

| 字段 | 旧值（违规） | 新值（schema 合法） |
|------|------------|------------------|
| `kind` | `ask_user` | `question_request` |
| `selection_mode`（有选项） | `single` | `single_select` |
| `selection_mode`（无选项） | `free_text` | `text` |
| `default` | 裸字符串字段 | `default_option_ids: [string]` |
| `interaction_id` | `ix-{id}-ask` | `ix-{date8}-{seq6}`（符合 pattern） |

**`seeyue-mcp/src/tools/input_request.rs`** — `project_input_as_interaction`:

| 字段 | 旧值（违规） | 新值（schema 合法） |
|------|------------|------------------|
| `selection_mode` | `free_text` | `path`/`secret`/`text`（按 kind 映射） |
| `input_kind`/`language`/`example` | 非 schema 字段 | 合并到 `detail`（nullable string） |
| `interaction_id` | `ix-{id}-input` | `ix-{date8}-{seq6}`（符合 pattern） |

**`seeyue-mcp/tests/interaction_migration.rs`** — 6 个测试同步更新：
- 断言 `kind == "question_request"`
- 断言 `selection_mode == "single_select"` 或 `"text"`
- 断言 `default_option_ids` 数组
- 断言 `interaction_id` pattern（`ix-` 前缀 + 8位日期 + 序号）
- 断言 `selection_mode == "path"` for file_path kind
- 断言 `detail` 包含 example 内容

### Gate 结果

```
cargo test --test interaction_migration --quiet  →  6 passed
node tests/runtime/run-interaction-fixtures.cjs --case legacy-to-interaction-projection  →  CASE_PASS
```

---

## P2-N5 修复：elicitation-first orchestration

### 问题根因

三处缺口：
1. `selectStrategy` 未探测 `seeyue-mcp/target/` 路径，导致 local_presenter 策略在实际仓库下不生效
2. `handleElicitation`/`handleTextFallback` 用错误签名调用 `writeResponse(rootDir, interactionId, response)`，实际 API 是 `writeResponse(rootDir, responseObj)`，导致持久化失败
3. pre-resolution 状态（`elicitation_pending`/`text_fallback_pending`）不是 response schema 合法值，不应写入 response store

### 修复内容

**`scripts/runtime/interaction-dispatch.cjs`**:
- `selectStrategy` binary 探测新增 `seeyue-mcp/target/{debug,release}/sy-interact{.exe,}` 路径
- `handleElicitation`：移除错误的 `writeResponse` 调用，只返回内存结果对象；客户端所有权不在 response store
- `handleTextFallback`：同上，移除错误的 `writeResponse` 调用
- 移除未使用的 `writeResponse` import

**`seeyue-mcp/src/tools/interaction_strategy.rs`** — `find_presenter_binary`:
- 新增 `workspace/seeyue-mcp/target/{debug,release}/sy-interact{.exe,}` 候选路径
- 与 JS 侧探测逻辑保持一致

**`tests/runtime/run-interaction-fixtures.cjs`**:
- `orchestration-elicitation-dispatch`：补 `readResponse === null` 断言（pre-resolution 不写 store）
- `orchestration-text-fallback-dispatch`：补 `readResponse === null` 断言

### 实测验证

```
# seeyue-mcp/target/debug/sy-interact.exe 存在时：
selectStrategy(process.cwd(), {})  →  { strategy: 'local_presenter', reason: 'sy-interact binary found' }

# elicitation dispatch 后：
readResponse(root, req.interaction_id)  →  null（正确，pre-resolution 不写 store）
```

### Gate 结果

```
cargo test --test interaction_mcp_client --quiet  →  5 passed
node tests/runtime/run-interaction-fixtures.cjs   →  17/17 CASE_PASS, INTERACTION_FIXTURES_PASS
```

---

## 文件变更列表

| 文件 | 变更类型 |
|------|--------|
| `seeyue-mcp/src/tools/ask_user.rs` | 修改：schema 对齐（kind/selection_mode/default_option_ids/interaction_id） |
| `seeyue-mcp/src/tools/input_request.rs` | 修改：schema 对齐（selection_mode/detail/interaction_id） |
| `seeyue-mcp/src/tools/interaction_strategy.rs` | 修改：新增 seeyue-mcp/target/ binary 探测路径 |
| `seeyue-mcp/src/tools/approval.rs` | 修改：journal payload clone 修复（避免 move 语义问题） |
| `seeyue-mcp/tests/interaction_migration.rs` | 修改：断言对齐 schema 合法值 |
| `seeyue-mcp/tests/interaction_mcp_client.rs` | 修改：移除硬 false 断言，补分发路径测试 |
| `scripts/runtime/interaction-dispatch.cjs` | 新建：orchestration 入口，修复 writeResponse 调用 |
| `tests/runtime/run-interaction-fixtures.cjs` | 修改：补 P2-N5 orchestration cases + readResponse 持久化断言 |
