# Symbol-First & MCP Dispatch Implementation Task List

Status: draft
Scope: Implement the three source-of-truth documents approved as design baseline
Plan type: TDD execution task list (red → green → refactor per node)
Estimated total: ~6-8 working days
Shell contract: 所有 verify.cmd / red_cmd / green_cmd 均以 bash 执行（Windows 下: bash -lc "<cmd>"）；不兼容 PowerShell / cmd.exe

---

## 1. Scope Gate

### Source of Truth (三份基线文档)

- `docs/symbol-first-gap-analysis.md` — 7 Gap + 补齐路线 A/B/C
- `docs/symbol-first-dispatch-design.md` — 4 迁移阶段 M1-M4
- `docs/symbol-first-north-star.md` — §8.7 多语言 discover_server 补全

### In Scope

**阶段 A — 导航基础（P0）**
- `sy_get_symbols_overview` 工具（LSP 主路径 + tree-sitter 降级）
- `sy_find_symbol` 工具（name_path 路由）
- `discover_server()` 补全（10 种语言）
- `.seeyue/index.json` 项目符号索引快照

**阶段 B — 编辑精化（P1）**
- `sy_find_referencing_symbols` 工具
- `sy_replace_symbol_body` 工具
- `sy_insert_after_symbol` / `sy_insert_before_symbol` 工具

**阶段 M — MCP Dispatch 重构（无功能变更）**
- `tools/metadata.rs` ToolMetadata 注册表
- `server/dispatch.rs` 统一 dispatch 入口
- `server/compat.rs` 客户端兼容层
- `AppState` active_tools HashSet + Active Filter

### Out of Scope

- `sy_rename_symbol`（阶段 C，P2，按需）
- tree-sitter grammar crate 新增（阶段 A tree-sitter 降级路径已有 5 种，扩展为独立任务）
- GUI dashboard
- Serena 自动下载机制实现（保持 PATH 探测 + hint）

### Baseline Constraints

- TDD 强制：每个节点必须先写失败测试（red），再写实现（green）
- `cargo test` 必须全绿才能关闭节点
- `seeyue-mcp` stdout 保持 JSON-RPC 洁净
- LSP session 失败不 panic，必须降级到 tree-sitter 路径
- 坐标系统契约（UTF-8 行列）遵循 `docs/symbol-first-gap-analysis.md` §附
- 新增文件遵循现有目录约定（`tools/xxx.rs` + `params/xxx.rs` + `tests/xxx.rs`）

---

## 2. Baseline Snapshot

现有实现面（与本计划相关）：

- `seeyue-mcp/src/lsp/mod.rs` — LspSessionPool, request_definition, request_references, request_hover, discover_server（5 种语言）
- `seeyue-mcp/src/treesitter/languages.rs` — TsLanguage（5 种）, detect_language, grammar_for
- `seeyue-mcp/src/tools/find_definition.rs` — 已有 go_to_definition 工具
- `seeyue-mcp/src/tools/find_references.rs` — 已有 find_references 工具
- `seeyue-mcp/src/tools/file_outline.rs` — 已有 file_outline 工具（基础版，无 name_path）
- `seeyue-mcp/src/tools/symbol_rename_preview.rs` — 已有重命名预览（非执行）
- `seeyue-mcp/src/server/` — 已有 dispatch/tools_core 拆分方向
- `seeyue-mcp/src/app_state.rs` — AppState
- `seeyue-mcp/src/error.rs` — ToolError 枚举

已知杠杆点：
- `request_references()` 已实现，B 阶段直接复用
- `LspSessionPool` 已有多语言并行管理，A1/A2 扩展 documentSymbol 方法即可
- `tools/mod.rs` 每新增工具补 `pub mod` 即可接入

---

## 3. Phase A — 导航基础（P0，~2 天）

Source of Truth: `docs/symbol-first-gap-analysis.md` §五 阶段 A

Phase Boundary:

```
entry_condition:
  - 三份基线文档已批准（当前状态）
  - seeyue-mcp cargo check 通过
  - 现有测试套件全绿

exit_gate:
  cmd: >
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
  pass_signal: exit 0
  note: run-interaction-fixtures.cjs 与 Phase A 无直接关联，移入总完成标志
```

### Phase A Node Summary

| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| A-N1 | lsp/mod.rs | 新增 request_document_symbols() | cargo test lsp_document_symbols | medium | [] |
| A-N2 | treesitter/symbols.rs | 提取符号树 + name_path 生成 | cargo test ts_symbols | low | [] |
| A-N3 | tools/get_symbols_overview.rs | sy_get_symbols_overview 工具（LSP主+ts降级） | cargo test symbol_overview | medium | [A-N1, A-N2] |
| A-N4 | tools/find_symbol.rs | sy_find_symbol 工具（name_path路由） | cargo test symbol_find | medium | [A-N3] |
| A-N5 | lsp/mod.rs:discover_server | 补全 10 种语言 match arm | cargo test lsp_discover | low | [] |
| A-N4b | tools/find_symbol.rs | sy_find_symbol 接入 index.json 加速层 | cargo test symbol_find_index | low | [A-N4, A-N6] |
| A-N6 | tools/project_index.rs | .seeyue/index.json 读写 + mtime 失效 | cargo test project_index | medium | [A-N2, A-N3] |
| A-N7 | hooks/session_start | SessionStart 触发增量索引更新 | cargo test session_index_update | low | [A-N6] |

### Phase A Detailed Nodes

#### A-N1
- `id`: A-N1
- `title`: LSP documentSymbol 底层接口
- `target`:
  - `seeyue-mcp/src/lsp/mod.rs`
  - `seeyue-mcp/tests/lsp_document_symbols.rs`（新建）
- `action`: 在 `LspSession` 上新增 `request_document_symbols(path, language_id, text)` 方法，发送 `textDocument/documentSymbol` 请求，解析返回的 `DocumentSymbol[]`（nested）或 `SymbolInformation[]`（flat），统一转换为内部 `LspSymbol { name, kind, range, children }` 结构。
- `why`: A1/A2 的 LSP 主路径基础，必须先有底层接口。
- `depends_on`: []
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test lsp_document_symbols -- --test-thread=1 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test lsp_document_symbols`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test lsp_document_symbols`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - Red: 测试调用不存在的 `request_document_symbols()`，编译失败即 red
  - Green: 实现方法，mock LSP response，验证解析正确
  - 测试用例：(1) DocumentSymbol 嵌套格式 (2) SymbolInformation flat 格式 (3) 空结果 (4) LSP 超时降级

#### A-N2
- `id`: A-N2
- `title`: tree-sitter 符号树提取 + name_path 生成
- `target`:
  - `seeyue-mcp/src/treesitter/symbols.rs`（新建）
  - `seeyue-mcp/tests/ts_symbols.rs`（新建）
- `action`: 实现 `extract_symbols(src: &str, lang: TsLanguage) -> Vec<TsSymbol>` 函数。`TsSymbol` 结构：`{ name, kind, start_line, end_line, children: Vec<TsSymbol> }`。实现 `to_name_path()` 方法生成 `ClassName/method_name` 格式，重载处理用 `[idx]` 后缀。
- `why`: LSP 不可用时的降级路径，也是 A-N6 索引快照的数据来源。
- `depends_on`: []
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test ts_symbols 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test ts_symbols`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test ts_symbols`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_notes`:
  - Red: 调用不存在的 `extract_symbols()`，编译失败
  - Green: 实现 Rust 文件的函数/impl 提取，至少覆盖 fn/struct/impl/trait
  - 测试用例：(1) 顶层函数 (2) impl 块内方法 (3) 重载（同名函数追加 [0][1]） (4) 嵌套 struct (5) Unicode 标识符
  - CRLF 测试：Windows 换行符文件的行号计算

#### A-N3
- `id`: A-N3
- `title`: `sy_get_symbols_overview` MCP 工具
- `target`:
  - `seeyue-mcp/src/tools/get_symbols_overview.rs`（新建）
  - `seeyue-mcp/src/params/get_symbols_overview.rs`（新建）
  - `seeyue-mcp/tests/symbol_overview.rs`（新建）
  - `seeyue-mcp/src/tools/mod.rs`（补 pub mod）
  - `seeyue-mcp/src/lib.rs`（补工具注册）
- `action`: 实现 `sy_get_symbols_overview(relative_path, depth)` 工具。策略：先尝试 LspSessionPool.get_or_start + request_document_symbols；若失败降级到 extract_symbols（tree-sitter）。输出 `{ symbols, source: "lsp"|"syntax" }`。
- `why`: 整个 symbol-first 链路的入口，A-N4 依赖此节点。
- `depends_on`: [A-N1, A-N2]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test symbol_overview 2>&1 | grep -E 'FAILED|error'`
- `green_cmd`: `cd seeyue-mcp && cargo test --test symbol_overview`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test symbol_overview && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - Red: 调用不存在的工具，编译失败
  - Green 分步：(1) LSP 主路径（mock LspSession） (2) 降级路径（关闭 LSP，验证 source=syntax） (3) depth 参数过滤
  - 测试用例：(1) LSP 可用 → source=lsp (2) LSP 不可用 → source=syntax (3) depth=0 只返回顶层 (4) depth=1 返回子符号 (5) 文件不存在 → ToolError::FileNotFound (6) 不支持语言 → source=syntax

#### A-N4
- `id`: A-N4
- `title`: `sy_find_symbol` MCP 工具
- `target`:
  - `seeyue-mcp/src/tools/find_symbol.rs`（新建）
  - `seeyue-mcp/src/params/find_symbol.rs`（新建）
  - `seeyue-mcp/tests/symbol_find.rs`（新建）
- `action`: 实现 `sy_find_symbol(name_path_pattern, relative_path, substring_matching, include_body, depth)`。基础实现：调用 get_symbols_overview 建立内存符号索引，name_path 前缀/子串匹配，按需读 body（行范围）。输出含 source 字段。（index.json 加速层在 A-N4b 中接入）
- `why`: name_path 路由是 symbol-first 链路的核心定位机制。
- `depends_on`: [A-N3]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test symbol_find 2>&1 | grep -E 'FAILED|error'`
- `green_cmd`: `cd seeyue-mcp && cargo test --test symbol_find`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test symbol_find && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - 测试用例：(1) 精确匹配 "UserSession/validate" (2) substring_matching=true 返回多个 (3) 全局搜索跨文件 (4) include_body 返回函数体 (5) 未找到 → 空列表 (6) CRLF 行范围正确

#### A-N5
- `id`: A-N5
- `title`: `discover_server()` 补全 10 种语言
- `target`:
  - `seeyue-mcp/src/lsp/mod.rs`
  - `seeyue-mcp/tests/lsp_discover.rs`（新建）
- `action`: 补全 match arm：c/cpp→clangd, kotlin→kotlin-language-server, css→vscode-css-language-server, vue→vue-language-server, bash/sh/shell→bash-language-server, markdown/md→marksman, json→vscode-json-language-server, toml→taplo, yaml/yml→yaml-language-server, bat→LspNotAvailable。同步更新 language_id()。每项带三要素 hint。
- `depends_on`: []
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test lsp_discover 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test lsp_discover`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test lsp_discover && cargo check`
- `risk_level`: low
- `tdd_notes`:
  - 测试用例：每种语言 hint 非空；bat 返回 LspNotAvailable；AGENT_EDITOR_LSP_CMD 覆盖优先；未知语言 hint 含逃生舱说明

#### A-N6
- `id`: A-N6
- `title`: `.seeyue/index.json` 项目符号索引
- `target`:
  - `seeyue-mcp/src/tools/project_index.rs`（新建）
  - `seeyue-mcp/tests/project_index.rs`（新建）
- `action`: 实现 ProjectIndex { build, update, query, load }。JSON 格式遵循 gap-analysis §A4 快照格式。mtime 驱动增量更新。写入用 atomic write（先写 .tmp 再 rename），避免并发写入损坏。
- `depends_on`: [A-N2, A-N3]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test project_index; if [ $? -eq 0 ]; then echo RED_FAIL; fi`
- `green_cmd`: `cd seeyue-mcp && cargo test --test project_index`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test project_index && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - 测试用例：(1) build 生成合法 json (2) load 反序列化 (3) update 只改变更文件 (4) mtime 未变不重建 (5) index.json 不存在返回 empty (6) workspace_root 变更全量重建 (7) 自动创建 .seeyue/ 目录 (8) atomic write：.tmp 存在但 rename 失败时原文件未损坏
#### A-N4b
- `id`: A-N4b
- `title`: sy_find_symbol 接入 index.json 加速层
- `target`:
  - `seeyue-mcp/src/tools/find_symbol.rs`（修改）
  - `seeyue-mcp/tests/symbol_find_index.rs`（新建）
- `action`: 在 sy_find_symbol 查询路径前插入 index 加速层：(1) 优先查 ProjectIndex.query（.seeyue/index.json）；(2) cache hit 直接返回，跳过 get_symbols_overview；(3) cache miss 时退回 A-N4 原有路径。此节点不改变工具接口，仅优化内部查询路径。
- `why`: A-N4 基础实现不依赖 A-N6，DAG 可独立推进；A-N4b 作为独立补丁节点，在 A-N6 完成后接入，保持 DAG 无环。
- `depends_on`: [A-N4, A-N6]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test symbol_find_index 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test symbol_find_index`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test symbol_find_index && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_notes`:
  - 测试用例：(1) index.json 存在且命中 → 不调用 get_symbols_overview (2) index.json 不存在 → 退回原路径 (3) index 命中后结果与原路径一致 (4) index 过期（mtime 变化）→ cache miss，走原路径


#### A-N7
- `id`: A-N7
- `title`: SessionStart hook 触发增量索引更新
- `target`:
  - `seeyue-mcp/src/hooks/session_start.rs`（修改，Hook 链路入口）
  - `seeyue-mcp/tests/session_index_update.rs`（新建）
- `action`: 在 Hook 链路入口 `hooks/session_start.rs` 中触发增量索引更新（非 MCP tool `sy_session_start`，两者职责不同）。通过后台线程（非阻塞触发）运行 ProjectIndex::update，静默失败（warn 日志到 stderr），不阻塞 hook 响应。Hook 路径：hooks/router.rs → hooks/session_start.rs。注：sy-hook 是同步二进制，不使用 async spawn，改用 std::thread::spawn 后台线程实现非阻塞。
- `why`: Hook 路径在会话真正启动时触发，比 MCP tool 路径更可靠；sy_session_start 工具由 agent 主动调用，不适合作为自动触发点。
- `depends_on`: [A-N6]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test session_index_update; if [ $? -eq 0 ]; then echo RED_FAIL; fi`
- `green_cmd`: `cd seeyue-mcp && cargo test --test session_index_update`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test session_index_update && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_notes`:
  - 测试用例：(1) hook 触发后 index 被更新 (2) index 更新失败时 hook 正常完成（不中断会话） (3) 无 workspace 时跳过索引更新 (4) 验证触发点在 hooks/session_start.rs，非 tools/hooks.rs

---

## 4. Phase B — 编辑精化（P1，~2 天）

Source of Truth: `docs/symbol-first-gap-analysis.md` §五 阶段 B

```
entry_condition:
  - Phase A 所有节点 cargo test 全绿
exit_gate:
  cmd: cd seeyue-mcp && cargo test --test symbol_references && cargo test --test symbol_replace && cargo test --test symbol_insert
```

### Phase B Node Summary

| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| B-N1 | find_referencing_symbols.rs | sy_find_referencing_symbols | cargo test symbol_references | medium | [A-N4] |
| B-N2 | replace_symbol_body.rs | sy_replace_symbol_body | cargo test symbol_replace | high | [A-N4] |
| B-N3 | insert_symbol.rs | sy_insert_after/before_symbol | cargo test symbol_insert | medium | [B-N2] |

### Phase B Detailed Nodes

#### B-N1
- `id`: B-N1
- `title`: `sy_find_referencing_symbols` MCP 工具
- `target`:
  - `seeyue-mcp/src/tools/find_referencing_symbols.rs`（新建）
  - `seeyue-mcp/src/params/find_referencing_symbols.rs`（新建）
  - `seeyue-mcp/tests/symbol_references.rs`（新建）
- `action`:
  1. sy_find_symbol(name_path) → (path, line, col)
  2. LspSession.request_references() → Vec<LspLocation>（已有）
  3. 对每个 LspLocation 反查最内层 enclosing symbol
  4. 反查规则：最内层 enclosing（start_line <= ref_line <= end_line，深度最大）；无 enclosing → name_path="<file>"；宏展开（无符号树对应）→ name_path="<macro>" + 原始代码片段，不静默丢弃
  5. 输出：{ references: [ { name_path, relative_path, line, snippet } ] }
- `depends_on`: [A-N4]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test symbol_references 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test symbol_references`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test symbol_references && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - 测试用例：(1) 普通函数调用反查到调用方函数 (2) 顶层调用 → name_path="<file>" (3) 宏展开引用 → name_path="<macro>" 含代码片段 (4) 无引用 → 空列表 (5) LSP 不可用 → ToolError::LspNotAvailable

#### B-N2
- `id`: B-N2
- `title`: `sy_replace_symbol_body` MCP 工具
- `target`:
  - `seeyue-mcp/src/tools/replace_symbol_body.rs`（新建）
  - `seeyue-mcp/src/params/replace_symbol_body.rs`（新建）
  - `seeyue-mcp/tests/symbol_replace.rs`（新建）
- `action`:
  1. sy_find_symbol(name_path, include_body=true) → start_line / end_line
  2. 按坐标契约读文件（保留原始行尾 CRLF/LF）
  3. 替换 [start_line, end_line] 区间为新 body
  4. Atomic write：先写 .tmp 文件，rename 覆盖原文件
  5. 输出：{ success: bool, lines_changed: i32 }
  注：body 必须包含完整签名行
- `depends_on`: [A-N4]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test symbol_replace 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test symbol_replace`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test symbol_replace && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_notes`:
  - Red: 先写失败测试：调用不存在函数
  - Green 分步：(1) 简单函数替换 (2) impl 方法替换 (3) Unicode 多字节字符文件 (4) CRLF 文件写回保留 CRLF (5) atomic write（模拟写中断）
  - 测试用例：(1) 正常替换 lines_changed 正确 (2) name_path 不存在 → ToolError (3) body 不含签名行 → ToolError::InvalidBody (4) 文件只读 → IoError (5) 替换后文件内容逐行验证

#### B-N3
- `id`: B-N3
- `title`: `sy_insert_after_symbol` / `sy_insert_before_symbol` MCP 工具
- `target`:
  - `seeyue-mcp/src/tools/insert_symbol.rs`（新建）
  - `seeyue-mcp/src/params/insert_symbol.rs`（新建）
  - `seeyue-mcp/tests/symbol_insert.rs`（新建）
- `action`: 实现两个工具共享一个实现模块。insert_after：在 end_line+1 插入；insert_before：在 start_line-1 插入（不处理前导注释，文档已说明）。同样 atomic write。
- `depends_on`: [B-N2]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test symbol_insert 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test symbol_insert`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test symbol_insert && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - 测试用例：(1) insert_after 在正确行插入 (2) insert_before 在正确行插入 (3) 末尾符号 insert_after (4) CRLF 文件写回 (5) atomic write

---

## 5. Phase M — MCP Dispatch 重构（无功能变更，~2 天）

Source of Truth: `docs/symbol-first-dispatch-design.md` §八 迁移路线

重构原则：**每个节点完成后 cargo test 必须全绿，行为不变**。

```
entry_condition:
  - 可与 Phase A 并行启动（无功能依赖）
  - 或 Phase A 完成后串行执行
exit_gate:
  cmd: >
    cd seeyue-mcp && cargo test
    && node tests/e2e/run-engine-conformance.cjs
  pass_signal: exit 0
  note: 行为不变 — 测试套件与重构前完全一致
```

### Phase M Node Summary

| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| M-N1 | tools/metadata.rs | ToolMetadata 注册表 | cargo check + cargo test | low | [] |
| M-N2 | server/schema.rs | tools/list 从 registry() 生成 | cargo test + e2e tools/list | low | [M-N1] |
| M-N3 | server/dispatch.rs | 统一 dispatch 入口 | cargo test（行为不变） | medium | [M-N1] |
| M-N4 | server/compat.rs | 客户端兼容层 | cargo test + compat unit tests | low | [M-N2] |
| M-N5 | app_state.rs | active_tools HashSet + Active Filter | cargo test active_filter | medium | [M-N3] |

### Phase M Detailed Nodes

#### M-N1
- `id`: M-N1
- `title`: ToolMetadata 注册表（元数据层）
- `target`:
  - `seeyue-mcp/src/tools/metadata.rs`（新建）
  - 所有现有 `tools/xxx.rs`（各加 `pub const METADATA: ToolMetadata`）
- `action`:
  1. 定义 `ToolMetadata` struct + `ToolCategory` 枚举（遵循 mcp-dispatch §3.1）
  2. 定义自由函数 `registry()` + `register_all_tools()`（遵循 §3.2）
  3. `impl ToolMetadata { get(), is_active() }` 关联函数（遵循 §3.2 修订版）
  4. 为现有 58 个工具各加 `pub const METADATA`（最小化字段：name, description, category, read_only, destructive, active_by_default）
  5. `cargo check` 确认编译通过
- `why`: 所有后续节点的基础，消除 hooks.spec.yaml 中重复工具描述。
- `depends_on`: []
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test metadata_registry 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test metadata_registry`
- `verify.cmd`: `cd seeyue-mcp && cargo check && cargo test --test metadata_registry`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_notes`:
  - 测试用例：(1) registry() 返回非空 (2) 所有工具名唯一 (3) get("read_file") 返回正确元数据 (4) is_active("read_file", empty_set) = true（active_by_default） (5) is_active("sy_find_symbol", empty_set) = false (6) is_active("sy_find_symbol", {"sy_find_symbol"}) = true

#### M-N2
- `id`: M-N2
- `title`: tools/list 响应从 registry() 自动生成
- `target`:
  - `seeyue-mcp/src/server/schema.rs`（修改或新建）
  - `seeyue-mcp/tests/schema_tools_list.rs`（新建）
- `action`:
  1. 实现 `generate_tools_list()` 调用 `registry().values()`（遵循 §5.1）
  2. 每个工具生成 `McpAnnotations { read_only_hint, destructive_hint }`
  3. `generate_input_schema(name)` 保持当前半自动实现（match name → schemars）
  4. 替换 main.rs / tools_core.rs 中手写的工具描述列表
  5. 运行 e2e 验证 tools/list 响应与重构前完全一致
- `depends_on`: [M-N1]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test schema_tools_list 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test schema_tools_list`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test schema_tools_list && node tests/e2e/run-engine-conformance.cjs`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_notes`:
  - 测试用例：(1) tools/list 包含所有 58 个工具名 (2) read_file 的 read_only_hint=true (3) write_file 的 destructive_hint=false (4) 工具名唯一，无重复 (5) MCP annotations 格式合法

#### M-N3
- `id`: M-N3
- `title`: server/dispatch.rs 统一 dispatch 入口
- `target`:
  - `seeyue-mcp/src/server/dispatch.rs`（新建）
  - `seeyue-mcp/src/server/tools_core.rs`（逐步迁移 match arm）
  - `seeyue-mcp/tests/dispatch_routing.rs`（新建）
- `action`:
  1. 实现 `dispatch_tool(name, raw_params, state)` 遵循 §4.1
  2. 读锁在局部块内 drop，不跨 await 边界（遵循修订版 §4.1）
  3. `route_tool()` 中迁移所有 match arm（从 tools_core.rs）
  4. main.rs call_tool 处理改为调用 `dispatch_tool()`
  5. 定义 `McpError` 枚举（§4.2），实现 JSON-RPC error 序列化
  6. 全量 `cargo test` 确认行为不变
- `depends_on`: [M-N1]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test dispatch_routing 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test dispatch_routing && cargo test`
- `verify.cmd`: `cd seeyue-mcp && cargo test && node tests/e2e/run-engine-conformance.cjs`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - 迁移策略：每次迁移 5-10 个工具的 match arm，`cargo test` 全绿后继续
  - 测试用例：(1) 已知工具路由成功 (2) 未知工具 → McpError::MethodNotFound (3) 参数解析失败 → McpError::InvalidParams (4) workspace 缺失 → McpError::WorkspaceRequired (5) 错误响应含 hint 字段

#### M-N4
- `id`: M-N4
- `title`: server/compat.rs 客户端兼容层
- `target`:
  - `seeyue-mcp/src/server/compat.rs`（新建）
  - `seeyue-mcp/tests/compat_schema.rs`（新建）
- `action`:
  1. 定义 `ClientType { Claude, OpenAI, Gemini, Unknown }`（§5.2 修订版）
  2. 实现 `sanitize_for_client(schema, client_type)`：Claude 无修改；OpenAI 移除 additionalProperties/$schema/const；Gemini 展平 anyOf/nullable；Unknown 最保守兼容集
  3. AppState 增加 `client_type: ClientType`
  4. tools/list 响应路径调用 sanitize_for_client
- `depends_on`: [M-N2]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test compat_schema; if [ $? -eq 0 ]; then echo RED_FAIL; fi`
- `green_cmd`: `cd seeyue-mcp && cargo test --test compat_schema`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test compat_schema && cargo check`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_notes`:
  - 测试用例：(1) Claude → schema 不变 (2) OpenAI → additionalProperties 被移除 (3) Gemini → anyOf 展平 (4) Unknown → 两种修改都执行 (5) 原始 schema 不被 mutate（传入副本）

#### M-N5
- `id`: M-N5
- `title`: active_tools HashSet + Active Filter
- `target`:
  - `seeyue-mcp/src/app_state.rs`
  - `seeyue-mcp/src/server/dispatch.rs`
  - `seeyue-mcp/tests/active_filter.rs`（新建）
- `action`:
  1. AppState.active_tools 改为 `Arc<RwLock<HashSet<String>>>`（§6.1 修订版）
  2. dispatch_tool() 局部块读锁 + is_active() 检查（§4.1 修订版）
  3. **server 启动时**在 main.rs AppState 构建阶段读取 `.ai/workflow/capabilities.yaml` 的 `active_tools` 列表，初始化 AppState.active_tools（HashSet）；注意：workflow/capabilities.yaml 是 schema 文件不可修改，.ai/workflow/capabilities.yaml 是运行时文件
  4. 不经 hook 进程（hooks/session_start.rs 属于 sy-hook 独立进程，无法访问 server 进程的 AppState 内存）；若需运行时刷新，使用 dispatch 懒加载：dispatch_tool() 首次调用时若 active_tools 为空则重新读取
  5. 被禁用工具调用 → McpError::ToolDisabled（含提示，不从 tools/list 消失）
- `depends_on`: [M-N3]
- `tdd_required`: true
- `red_cmd`: `cd seeyue-mcp && cargo test --test active_filter 2>&1 | grep FAILED`
- `green_cmd`: `cd seeyue-mcp && cargo test --test active_filter`
- `verify.cmd`: `cd seeyue-mcp && cargo test --test active_filter && cargo test`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_notes`:
  - 测试用例：(1) active_by_default=true 工具无需 active_tools 即可调用 (2) active_by_default=false 工具被调用 → ToolDisabled (3) active_tools 含该工具 → 可调用 (4) HashSet 去重（重复项无副作用） (5) server 启动时从 .ai/workflow/capabilities.yaml 正确初始化 active_tools (6) .ai/workflow/capabilities.yaml 不存在时 active_tools 为空集（仅 active_by_default 生效） (7) active_tools 读锁不跨 await

---

## 6. 全局 TDD 规则

### 6.1 Red-Green-Refactor 执行规范

```
Red 阶段：
  - 先写测试文件（tests/xxx.rs）
  - 调用尚不存在的函数/工具
  - cargo test 必须报 compile error 或 test FAILED
  - 禁止在 Red 阶段写任何实现代码

Green 阶段：
  - 写最小实现使测试通过
  - 不做超出测试要求的优化
  - cargo test --test xxx 全绿

Refactor 阶段：
  - 在测试保护下改善代码质量
  - cargo test 必须保持全绿
  -
  每次 refactor 后运行全量 cargo test

节点关闭条件：
  - verify.cmd 退出码 0
  - 无 TODO/FIXME/unwrap()（unsafe 除外需注释说明）
  - 新增代码覆盖率（关键路径）不低于已有模块
```

### 6.2 坐标系统测试规范

所有涉及文件行列操作的节点（A-N2, A-N3, A-N4, B-N2, B-N3）必须包含：

```
坐标测试矩阵：
  [ ] LF 文件（Unix）行号正确
  [ ] CRLF 文件（Windows）行号正确，写回保留 CRLF
  [ ] 含中文/emoji 多字节字符文件，列号基于 UTF-8 字符
  [ ] LSP UTF-16 offset 转换正确（utf16_to_char_idx）
  [ ] 嵌套符号 body range 不包含下一个符号的前导注释
```

### 6.3 降级路径测试规范

所有双路径工具（A-N3, A-N4）必须包含：

```
降级测试矩阵：
  [ ] LSP 正常 → source=lsp，结果语义正确
  [ ] LSP 启动失败 → source=syntax，结果语法级
  [ ] LSP 超时（>5s）→ source=syntax，不挂起
  [ ] 不支持语言 → source=syntax + 安装 hint
  [ ] source 字段始终存在（lsp 或 syntax，无其他值）
```

### 6.4 Atomic Write 测试规范

涉及文件写回的节点（B-N2, B-N3）必须包含：

```
Atomic write 测试：
  [ ] 写入成功后原文件内容正确
  [ ] 写中断（模拟 .tmp 存在但 rename 失败）→ 原文件未损坏
  [ ] 同一文件并发写入 → 最终一致（最后一次 wins）
```

---

## 7. 节点依赖图

```
阶段 A（P0，可并行启动 A-N1/A-N2/A-N5）：

  A-N1 (lsp documentSymbol) ──┐
                               ├──→ A-N3 (get_symbols_overview) ──→ A-N4 (find_symbol)
  A-N2 (ts symbols)       ──┘                                                 │
                                                                              └──→ A-N4b (find_symbol+index) ←── A-N6 (project_index) ──→ A-N7

  A-N5 (discover_server)  ── 独立，与 A-N1/A-N2 并行
  注：A-N6 依赖 A-N2（数据来源）和 A-N3（query接口），见 Node Summary 表

阶段 B（P1，依赖 A-N4）：

  A-N4 ──→ B-N1 (find_referencing_symbols)
       ──→ B-N2 (replace_symbol_body) ──→ B-N3 (insert_symbol)

阶段 M（可与 A 并行，无功能依赖）：

  M-N1 (metadata) ──→ M-N2 (schema) ──→ M-N4 (compat)
               └──→ M-N3 (dispatch) ──→ M-N5 (active_filter)
```

---

## 8. 风险与缓解

| 风险 | 节点 | 缓解措施 |
|------|------|----------|
| LSP documentSymbol 返回格式不一致（nested vs flat） | A-N1 | 测试覆盖两种格式，统一转换层 |
| CRLF 写回损坏文件 | B-N2, B-N3 | 坐标测试矩阵强制覆盖 |
| 宏展开引用静默丢失 | B-N1 | `<macro>` fallback 测试用例强制 |
| M-N3 迁移 match arm 时行为回归 | M-N3 | 每批 5-10 个 arm 迁移后全量 cargo test |
| active_tools 读锁跨 await 导致死锁 | M-N5 | 局部块 drop 模式 + 测试用例验证 |
| LSP 超时挂起会话 | A-N3 | 5s timeout + 降级，测试矩阵覆盖 |
| index.json 并发写入损坏 | A-N6 | atomic write（.tmp rename）|

---

## 9. 完成标志

### Phase A 完成标志

```bash
cd seeyue-mcp
cargo test --test lsp_document_symbols    # A-N1
cargo test --test ts_symbols              # A-N2
cargo test --test symbol_overview         # A-N3
cargo test --test symbol_find             # A-N4
cargo test --test symbol_find_index       # A-N4b
cargo test --test lsp_discover            # A-N5
cargo test --test project_index           # A-N6
cargo test --test session_index_update    # A-N7
cargo test                                # 全量回归
```

### Phase B 完成标志

```bash
cd seeyue-mcp
cargo test --test symbol_references       # B-N1
cargo test --test symbol_replace          # B-N2
cargo test --test symbol_insert           # B-N3
cargo test                                # 全量回归
```

### Phase M 完成标志

```bash
cd seeyue-mcp
cargo test --test metadata_registry       # M-N1
cargo test --test schema_tools_list       # M-N2
cargo test --test dispatch_routing        # M-N3
cargo test --test compat_schema           # M-N4
cargo test --test active_filter           # M-N5
cargo test                                # 全量回归
node tests/e2e/run-engine-conformance.cjs # e2e 行为不变验证
```

### 总完成标志

```bash
cd seeyue-mcp && cargo test
node tests/e2e/run-engine-conformance.cjs
node tests/runtime/run-interaction-fixtures.cjs
# 所有 exit 0
```

---

## 10. 文档关系

| 层级 | 文档 | 定位 |
|------|------|------|
| 北极星 | `docs/symbol-first-north-star.md` | 架构借鉴总纲 |
| Gap 分析 | `docs/symbol-first-gap-analysis.md` | symbol-first 能力差距与补齐路线 |
| 实施设计 | `docs/symbol-first-dispatch-design.md` | dispatch 层与工具元数据机器契约 |
| 任务清单 | `docs/symbol-first-task-list.md`（本文） | TDD 节点式实施任务 |
| 执行记录 | `docs/symbol-first-execution-record.md` | 逐节点验收证据沉淀 |

---

> 文档完成于 2026-03-19。所有 verify.cmd 在 seeyue-mcp/ 目录下执行。红绿重构顺序强制，不得跳过 Red 阶段。
