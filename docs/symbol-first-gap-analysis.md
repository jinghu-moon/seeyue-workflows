# Symbol-First Gap Analysis

> 基于 `docs/symbol-first-north-star.md` 的专题深化文档
> 分析日期：2026-03-19
> 目标：量化 seeyue-mcp 距离 Serena 式 symbol-first workflow 的具体差距，并给出补齐路线

---

## 一、为什么 symbol-first 是核心战略

Serena 对 agent 开发生产力的最核心贡献，不是它能接多少种语言服务器，而是它改变了 agent 定位代码的方式：

```
行号定位（fragile）       →  符号定位（stable）
"line 342 in auth.rs"   →  "UserSession/validate_token"
文件级搜索（广）          →  name_path 路由（精）
读全文再找目标            →  先 get_symbols_overview，再按需读 body
```

这是 agent 能力的质变，不是量变。行号随每次编辑漂移，符号路径（`ClassName/method_name`）在合理重构范围内稳定。对 seeyue-mcp 来说，这个能力决定了 agent 在大型代码库中的可用性上限。

---

## 二、Serena symbol-first workflow 的完整链路

### 2.1 Serena 的 7 步工作流

```
步骤 1  get_symbols_overview(path, depth=0)
        → 得到文件内所有顶层符号（函数/类/变量）的 name_path 列表
        → 源码：refer/serena-main/src/serena/tools/symbol_tools.py:GetSymbolsOverviewTool

步骤 2  find_symbol(name_path_pattern, substring_matching=True)
        → 精确定位目标符号（支持 "Class/method" 路由）
        → 返回：文件路径 + 行列 + 可选 body
        → 源码：refer/serena-main/src/serena/tools/symbol_tools.py:FindSymbolTool

步骤 3  find_referencing_symbols(name_path, relative_path)
        → 找所有调用该符号的地方（跨文件）
        → 底层：LSP textDocument/references
        → 源码：refer/serena-main/src/serena/tools/symbol_tools.py:FindReferencingSymbolsTool

步骤 4  find_symbol(include_body=True)
        → 读取目标符号 body（按需，不读全文）
        → 源码：refer/serena-main/src/serena/ls.py:SolidLanguageServer.get_document_symbols()

步骤 5  replace_symbol_body(name_path, relative_path, body)
        → 精确替换符号 body（行级定位，不动其余代码）
        → 源码：refer/serena-main/src/serena/tools/symbol_tools.py:ReplaceSymbolBodyTool

步骤 6  insert_after_symbol / insert_before_symbol
        → 在已知符号前后插入新代码
        → 源码：refer/serena-main/src/serena/tools/symbol_tools.py

步骤 7  rename_symbol(name_path, new_name)
        → LSP workspace/symbol + textDocument/rename 跨文件重命名
        → 源码：refer/serena-main/src/serena/tools/symbol_tools.py:RenameSymbolTool
```

### 2.2 稳定性来源

Serena 的符号定位稳定性来自三层机制（`refer/serena-main/src/solidlsp/ls.py`）：

| 层 | 机制 | 作用 |
|---|---|---|
| mtime 缓存失效 | `LSPFileBuffer` 比较文件修改时间 | 避免重复 open/parse |
| content_hash 驱动符号缓存 | `_document_symbols_cache[uri] = (hash, symbols)` | 文件内容不变则复用符号表 |
| overload_idx | `Symbol` 对象携带重载索引 | 处理 Java/C++ 函数重载 |

---

## 三、当前 seeyue-mcp 能力盘点

### 3.1 已有能力

| 能力 | 实现位置 | 说明 |
|------|----------|------|
| 行号跳转（go_to_definition） | `lsp/mod.rs:request_definition()` | LSP textDocument/definition |
| hover 信息 | `lsp/mod.rs:request_hover()` | LSP textDocument/hover |
| 引用查找 | `lsp/mod.rs:request_references()` | LSP textDocument/references |
| tree-sitter 符号提取 | `treesitter/` | 5 种语言（Rust/Python/TS/TSX/Go） |
| LSP session pool | `lsp/mod.rs:LspSessionPool` | 多语言并行会话管理 |
| LSP server 发现 | `lsp/mod.rs:discover_server()` | 支持 rust/ts/python/go |

### 3.2 缺失能力

#### Gap 1：无 `get_symbols_overview` 工具

**Serena 对应**：`GetSymbolsOverviewTool.apply(relative_path, depth)` — 返回文件内符号树

**当前状态**：
- tree-sitter 可以提取符号，但没有封装成 MCP 工具暴露给 agent
- LSP `textDocument/documentSymbol` 接口有底层实现，未暴露
- Agent 目前只能通过 `read_file` 读全文，再自行解析

**影响**：agent 无法快速了解文件结构，必须读全文，token 浪费严重

#### Gap 2：无 `find_symbol` 工具（name_path 路由）

**Serena 对应**：`FindSymbolTool.apply(name_path_pattern, substring_matching, include_body)`

**当前状态**：
- 没有 name_path 概念（`ClassName/method_name` 路由）
- 搜索依赖 `grep` 式文本匹配，不是符号树查询
- 无 `substring_matching` 支持（模糊符号名搜索）

**影响**：agent 无法精确定位符号，只能行号定位（脆弱）

#### Gap 3：无 `find_referencing_symbols` 工具

**Serena 对应**：`FindReferencingSymbolsTool` — 底层调用 `request_references()` 并将结果映射回符号

**当前状态**：
- `lsp/mod.rs:request_references()` **已有**底层实现
- 但没有封装成 MCP 工具，且结果是 `LspLocation`（行列），不是符号名
- 缺少 location → symbol name 的反查映射

**影响**：agent 无法做影响分析（"改这个函数会影响哪些调用方"）

#### Gap 4：无 `replace_symbol_body` 工具

**Serena 对应**：`ReplaceSymbolBodyTool` — 精确替换符号 body，不动其余代码

**当前状态**：
- 有 `multi_edit`，但基于行号范围，需要 agent 事先知道精确行号
- 无符号级编辑（不需要行号，只需要 name_path）

**影响**：agent 必须先定位行号再编辑，链路更长，更易出错

#### Gap 5：无 `insert_after_symbol / insert_before_symbol` 工具

**影响**：插入新函数/方法时需要手动定位行号，与 Gap 4 同类问题

#### Gap 6：无 `rename_symbol` 工具（跨文件）

**Serena 对应**：`RenameSymbolTool` — LSP workspace rename，自动更新所有引用

**当前状态**：
- 有 `symbol_rename_preview`（预览，不执行）
- 无 LSP `textDocument/rename` 执行路径

**影响**：重命名只能 grep+replace，无法保证语义正确性

#### Gap 7：`discover_server()` 覆盖语言不足

**当前**：仅支持 rust / typescript / tsx / javascript / jsx / python / go（7 种）

**实际语言栈需求**：vue / css / kotlin / c / c++ / shell / markdown / json / toml / yaml（共 10 种）

**影响**：目标语言栈中有 10 种语言无 LSP 支持，符号定位链路无法启动

---

## 四、Gap 优先级矩阵

| Gap | 影响范围 | 实现难度 | 优先级 |
|-----|----------|----------|--------|
| Gap 1：get_symbols_overview | 所有代码导航 | 低（tree-sitter 已有，包装即可） | **P0** |
| Gap 2：find_symbol（name_path） | 所有精确定位 | 中（需 name_path 路由层） | **P0** |
| Gap 3：find_referencing_symbols | 影响分析 | 低（底层 LSP 已有，包装+反查） | **P1** |
| Gap 4：replace_symbol_body | 符号级编辑 | 中（需 body range 计算） | **P1** |
| Gap 5：insert_after/before_symbol | 新代码插入 | 低（依赖 Gap 4 的 range 计算） | **P1** |
| Gap 6：rename_symbol | 跨文件重命名 | 中（LSP rename 协议） | **P2** |
| Gap 7：discover_server 扩展 | 多语言支持 | 低（补 match arm） | **P0**（与 A1/A2 同步推进，详见 `docs/symbol-first-north-star.md` §8.7） |

---

## 五、补齐路线

### 阶段 A：导航基础（P0，~1 天）

#### A1：`sy_get_symbols_overview` 工具

```
输入：relative_path: String, depth: u32 = 0
输出：{ symbols: [ { name, kind, name_path, line, children? } ] }

实现路径（LSP 为主，tree-sitter 降级兜底）：
  主路径：LSP textDocument/documentSymbol（语义级，准确）
    → lsp/mod.rs 新增 request_document_symbols()
    → 当 LSP session 存在且语言有 LSP 支持时使用
    → 输出标注 source: "lsp"
  降级路径：tree-sitter（语法级，无 LSP 时兜底）
    → treesitter/symbols.rs 提取符号树
    → 返回 TsSymbol { name, kind, start_line, end_line, children }
    → 输出标注 source: "syntax"（告知 agent 当前为语法级，非语义级）
    → 触发条件：LSP 不支持该语言 / LSP session 启动失败 / 超时

降级标注的意义：agent 可据此决定是否触发 LSP 安装提示，
或对 find_referencing_symbols 的结果持保留态度（语法级无跨文件引用）

关键设计：name_path 生成规则
  顶层符号：\"function_name\" 或 \"ClassName\"
  嵌套符号：\"ClassName/method_name\"（用 '/' 分隔，与 Serena 一致）
  重载处理：\"ClassName/method_name[0]\"（追加 0-based index）
```

#### A2：`sy_find_symbol` 工具

```
输入：
  name_path_pattern: String        # 如 \"UserSession/validate\" 或 \"validate\"
  relative_path: String = \"\"       # 空=全局搜索，否则限定文件/目录
  substring_matching: bool = false # 对 name_path 末段做子串匹配
  include_body: bool = false        # 是否返回符号 body 文本
  depth: u32 = 0                   # 是否返回子符号

输出：
  { symbols: [ { name_path, relative_path, line, column, body?, source: "lsp"|"syntax" } ] }

实现路径（LSP 为主，tree-sitter 降级）：
  主路径：LSP documentSymbol → 建立 name_path 索引 → 模式匹配
    → 与 A1 共享同一 LSP session，无额外开销
  降级路径：treesitter/symbols.rs 构建符号树 → name_path 匹配
    → source: "syntax" 标注，语义精度有限
  注：body 读取（include_body=true）始终走文件行范围，不依赖 LSP
```

#### A3：discover_server() 补全

与 A1/A2 同步推进。补全 10 种语言 match arm，每项带三要素 hint。
详见 `docs/symbol-first-north-star.md` §8.7。

#### A4：项目符号索引快照（冷启动加速）

**问题**：无此机制时，每次会话 agent 需重新对所有文件执行 get_symbols_overview
才能建立大图，在大型代码库中产生明显冷启动开销。

**方案**：轻量级持久化项目符号索引到 `.seeyue/index.json`

```
.seeyue/
  index.json          # 项目符号索引快照
  index.meta.json     # 快照元数据（生成时间、文件 mtime 哈希）
```

快照格式：

```json
{
  "generated_at": "2026-03-19T10:00:00Z",
  "workspace_root": "/abs/path",
  "files": {
    "src/auth.rs": {
      "mtime": 1742385600,
      "symbols": [
        { "name_path": "UserSession", "kind": "class", "line": 12 },
        { "name_path": "UserSession/validate_token", "kind": "method", "line": 34 }
      ]
    }
  }
}
```

缓存失效策略：
  - 文件级 mtime 比对：单文件变更只重建该文件条目
  - 全量重建触发条件：index.json 不存在 / workspace_root 变更 / 手动 --rebuild
  - 会话启动时：读取 index.json → 对比 dirty 文件 mtime → 增量更新

实现位置：
  - tools/project_index.rs — 建立/读取/增量更新索引
  - SessionStart hook 触发增量更新（静默，不阻塞会话启动）
  - sy_find_symbol 优先查 index.json，cache miss 再走 LSP/tree-sitter

与 Serena 的对应关系：
  Serena 用 .serena/memories/ 存储项目级记忆（自然语言描述）
  seeyue-mcp 用 .seeyue/index.json 存储结构化符号索引
  两者目标相同：消除每次会话的冷启动开销

---

### 阶段 B：编辑精化（P1，~2 天）

#### B1：`sy_find_referencing_symbols` 工具

```
输入：name_path: String, relative_path: String
输出：{ references: [ { name_path, relative_path, line, snippet } ] }

实现路径：
  1. find_symbol(name_path) → 得到目标符号的 (path, line, col)
  2. lsp/mod.rs:request_references(path, line, col) → Vec<LspLocation>（已有）
  3. 对每个 LspLocation，反查包含该行的符号（用 get_symbols_overview 结果索引）
  4. 返回调用方符号的 name_path + 代码片段

反查规则（命中多个符号时）：
  - 取**最内层 enclosing symbol**（start_line <= ref_line <= end_line 且深度最大）
  - 若无 enclosing symbol（如顶层语句），退回 file-level result：
      name_path = "<file>", relative_path = 文件路径
  - 若同一行有多个符号，取起始位置最早的符号
  - 宏展开场景特殊处理：Rust #[derive]、过程宏等生成的匿名代码
    在 LSP references 中出现但在 tree-sitter 符号树中无对应 enclosing symbol
    fallback：返回文件路径 + 宏调用行原始代码片段，name_path = "<macro>"
    不得静默丢弃此类引用（agent 需知道宏展开产生了调用）
```

#### B2：`sy_replace_symbol_body` 工具

```
输入：name_path: String, relative_path: String, body: String
输出：{ success: bool, lines_changed: i32 }

实现路径：
  1. find_symbol(name_path, include_body=true) → 得到 start_line / end_line
  2. 读文件，替换 [start_line, end_line] 区间为新 body
  3. 写回文件（atomic write，先写 tmp 再 rename）
  注：body 必须包含完整签名行（与 Serena 保持一致）
```

#### B3：`sy_insert_after_symbol` / `sy_insert_before_symbol` 工具

```
输入：name_path: String, relative_path: String, body: String
实现路径：
  依赖 B2 的 find_symbol range 计算
  insert_after：在 end_line+1 插入
  insert_before：在 start_line-1 插入（保留前导注释问题暂不处理）
```

---

### 附：坐标系统契约（B 阶段实现前必读）

`replace_symbol_body` / `insert_*_symbol` 涉及字节级文件操作，需统一三种坐标系：

| 坐标系 | 来源 | 单位 | 问题场景 |
|--------|------|------|----------|
| tree-sitter byte range | `Node.start_byte()` / `end_byte()` | UTF-8 字节偏移 | 多字节 Unicode 字符（中文/emoji）导致行列计算偏差 |
| LSP position | `textDocument/references` 返回 | UTF-16 code unit | Rust `String` 是 UTF-8，直接用 LSP offset 切片会 panic |
| Rust 行列 | 按 `\n` 分割后计数 | 行号（1-based），列（字符数） | CRLF（`\r\n`）在 Windows 上多出一个字节 |

**归一规则**（seeyue-mcp 内部统一标准）：

```
1. 内部存储：始终用 1-based 行号 + 1-based UTF-8 字符列
   （与 lsp/mod.rs 现有 LspLocation 保持一致）

2. 文件读写：按行分割使用 lines_with_endings()，保留原始行尾
   → 避免 CRLF 被 lines() 吃掉导致写回后换行符丢失

3. LSP position 转换：UTF-16 offset → UTF-8 字符 index
   fn utf16_to_char_idx(s: &str, utf16_offset: u64) -> usize
   → 遍历 char，累计 char.len_utf16()，直到达到 offset

4. tree-sitter byte range → 行列：
   fn byte_to_line_col(src: &str, byte: usize) -> (usize, usize)
   → 计数 src[..byte] 中的 '\n' 得到行号，最后一行内字节数得到列

5. 嵌套符号 body range：取 start_byte 到 end_byte，
   包含签名行，不包含下一个符号的前导注释
```

**CRLF 特别处理**：Windows 文件常见，写回时必须保留原行尾风格：
```rust
let line_ending = if src.contains("\r\n") { "\r\n" } else { "\n" };
```

---

### 阶段 C：语义重构（P2，按需）

#### C1：`sy_rename_symbol` 工具

```
输入：name_path: String, relative_path: String, new_name: String
输出：{ files_changed: [ { path, changes: i32 } ] }

实现路径：
  1. find_symbol → (path, line, col)
  2. LSP textDocument/prepareRename → 确认可重命名
  3. LSP textDocument/rename → WorkspaceEdit
  4. 应用 WorkspaceEdit（多文件批量替换）
  注：lsp/mod.rs 需新增 request_rename() 方法
```

---

## 六、与 symbol-first-north-star.md 的关系

本文档是 `docs/symbol-first-north-star.md` 第五节「直接借鉴的 6 个点」中「symbol-first 导航链路升级」的专项落地文档。

| 层级 | 文档 | 定位 |
|------|------|------|
| 北极星 | `docs/symbol-first-north-star.md` | 架构借鉴总纲 |
| 专题 gap 分析 | `docs/symbol-first-gap-analysis.md`（本文） | symbol-first 能力差距与补齐路线 |
| 实施设计 | `docs/symbol-first-dispatch-design.md` | dispatch 层与工具元数据机器契约 |

补齐完成后，seeyue-mcp 的 agent 代码定位链路将从：
```
read_file(全文) → grep 搜索 → 行号定位 → 编辑
```
升级为：
```
get_symbols_overview → find_symbol(name_path) → replace_symbol_body
```
这与 Serena 的 symbol-first workflow 对齐，是 agent 在大型代码库中可用性的关键跃升。

---

> 文档完成于 2026-03-19。所有 Gap 分析基于 `seeyue-mcp/src/` 实际代码，补齐路线按实现复杂度排序。