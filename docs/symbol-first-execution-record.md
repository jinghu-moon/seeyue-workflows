# Symbol-First & MCP Dispatch 执行记录

Status: in-progress
Task List: `docs/symbol-first-task-list.md`
Shell contract: bash（Windows 下 bash -lc "<cmd>"）

---

## 使用说明

每个节点执行完成后，在对应节点记录区填写：
- `completed_at`：完成时间
- `red_evidence`：Red 阶段截图或命令输出摘要
- `green_evidence`：Green 阶段 cargo test 输出摘要
- `verify_output`：verify.cmd 输出（exit 0 确认）
- `notes`：实际实现与设计文档的偏差、坑点、决策记录

节点状态标记：`[ ]` 未开始 / `[-]` 进行中 / `[x]` 已完成

---

## Phase A — 导航基础（P0）

exit_gate 验收命令：
```bash
cd seeyue-mcp
&& cargo test --test lsp_document_symbols
&& cargo test --test ts_symbols
&& cargo test --test symbol_overview
&& cargo test --test symbol_find
&& cargo test --test symbol_find_index
&& cargo test --test lsp_discover
&& cargo test --test project_index
&& cargo test --test session_index_update
&& cargo test
```

### [ ] A-N1 — LSP documentSymbol 底层接口

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  # cargo test --test lsp_document_symbols 输出（编译失败或 FAILED）
  ```
- `green_evidence`:
  ```
  # cargo test --test lsp_document_symbols 输出（全绿）
  ```
- `verify_output`:
  ```
  # verify.cmd 输出
  ```
- `deviations`: （与设计文档的偏差，无则填 none）
- `notes`:

---

### [ ] A-N2 — tree-sitter 符号树提取 + name_path 生成

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] A-N3 — `sy_get_symbols_overview` MCP 工具

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] A-N4 — `sy_find_symbol` MCP 工具（基础实现）

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] A-N5 — `discover_server()` 补全 10 种语言

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] A-N6 — `.seeyue/index.json` 项目符号索引

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] A-N4b — `sy_find_symbol` 接入 index.json 加速层

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] A-N7 — SessionStart hook 触发增量索引更新

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### Phase A exit_gate 验收

- `verified_at`:
- `output`:
  ```
  # 粘贴 exit_gate 命令完整输出
  ```
- `result`: [ ] PASS / [ ] FAIL

---

## Phase B — 编辑精化（P1）

exit_gate 验收命令：
```bash
cd seeyue-mcp
&& cargo test --test symbol_references
&& cargo test --test symbol_replace
&& cargo test --test symbol_insert
```

### [ ] B-N1 — `sy_find_referencing_symbols` MCP 工具

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] B-N2 — `sy_replace_symbol_body` MCP 工具

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] B-N3 — `sy_insert_after_symbol` / `sy_insert_before_symbol`

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### Phase B exit_gate 验收

- `verified_at`:
- `output`:
  ```
  ```
- `result`: [ ] PASS / [ ] FAIL

---

## Phase M — MCP Dispatch 重构

exit_gate 验收命令：
```bash
cd seeyue-mcp && cargo test
node tests/e2e/run-engine-conformance.cjs
```

### [ ] M-N1 — ToolMetadata 注册表

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] M-N2 — tools/list 从 registry() 自动生成

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] M-N3 — server/dispatch.rs 统一 dispatch 入口

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:
  - 迁移批次记录：
    - 批次 1（工具 1-10）：
    - 批次 2（工具 11-20）：
    - 批次 N：

---

### [ ] M-N4 — server/compat.rs 客户端兼容层

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### [ ] M-N5 — active_tools HashSet + Active Filter

- `started_at`:
- `completed_at`:
- `implementer`:
- `red_evidence`:
  ```
  ```
- `green_evidence`:
  ```
  ```
- `verify_output`:
  ```
  ```
- `deviations`:
- `notes`:

---

### Phase M exit_gate 验收

- `verified_at`:
- `output`:
  ```
  ```
- `result`: [ ] PASS / [ ] FAIL

---

## 总完成标志

```bash
cd seeyue-mcp && cargo test
node tests/e2e/run-engine-conformance.cjs
node tests/runtime/run-interaction-fixtures.cjs
```

- `verified_at`:
- `result`: [ ] PASS / [ ] FAIL

---

## 已知偏差汇总

> 执行过程中与设计文档的偏差，由各节点 deviations 字段汇总于此

| 节点 | 偏差描述 | 影响 | 处理方式 |
|------|----------|------|----------|
| | | | |

---

> 记录模板创建于 2026-03-19。按节点顺序填写，不得跳过 Red 阶段证据。
