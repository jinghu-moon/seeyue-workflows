# 文件编辑引擎（MCP-DEMO 分析与集成）

> 来源：`refer/MCP-DEMO/main.rs`，`refer/MCP-DEMO/lib.rs`，`refer/MCP-DEMO/Cargo.toml`
> 来源：`refer/MCP-DEMO/encoding_layer.rs`，`refer/MCP-DEMO/cache.rs`，`refer/MCP-DEMO/checkpoint.rs`
> 来源：`refer/MCP-DEMO/error.rs`，`refer/MCP-DEMO/path.rs`
> 来源：`refer/MCP-DEMO/V5-DESIGN.md`，`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md`
> 来源：`refer/MCP-DEMO/M1/`，`refer/MCP-DEMO/M2/`，`refer/MCP-DEMO/M3/`

---

## 1. 为什么选 Rust

来源：`refer/MCP-DEMO/V5-DESIGN.md` §一

```
Node.js MCP Server
  启动：300-800ms（Node.js 初始化 + JIT 预热）
  内存：~80MB
  Windows Defender：每次启动扫描 node_modules，无法缓存

Rust MCP Server（本方案）
  启动：< 30ms（单进程，无解释器）
  内存：< 8MB（no GC）
  二进制：单个 .exe，约 3-5MB（strip + lto）
  Windows Defender：一次扫描后缓存，后续几乎零开销
```

---

## 2. 实际依赖清单

来源：`refer/MCP-DEMO/Cargo.toml`（逐字核实）

```toml
[dependencies]
chardetng           = "0.1"       # 编码检测（Firefox 同款，Mozilla 出品）
encoding_rs         = "0.8"       # Servo 引擎编码解码（GBK/Shift-JIS/UTF-16 等）
thiserror           = "1"
anyhow              = "1"
serde               = { version = "1", features = ["derive"] }
serde_json          = "1"
sha2                = "0.10"      # 双 hash 校验
hex                 = "0.4"
similar             = { version = "2", features = ["text"] }  # Myers diff
rusqlite            = { version = "0.31", features = ["bundled"] }  # SQLite WAL
chrono              = { version = "0.4", features = ["serde"] }
unicode-normalization = "0.1"
tokio               = { version = "1", features = ["full"] }  # Windows IOCP
colored             = "2"
```

注：`main.rs` 另引入 `rmcp`（官方 Rust MCP SDK）和 `dirs`，seeyue-mcp 集成时需补充至 Cargo.toml。

---

## 3. 整体架构（基于真实代码）

来源：`refer/MCP-DEMO/main.rs`，`refer/MCP-DEMO/lib.rs`

两个入口：
- `lib.rs` → `Engine` 同步结构体（集成测试用，字段均为 pub）
- `main.rs` → `EditorServer`（async rmcp MCP Server，生产入口）

```
EditorServer（rmcp #[tool_router]）
  5 工具：read_file / write / edit / multi_edit / rewind
  AppState（Arc，全工具共享）
    workspace:  Arc<PathBuf>
    cache:      Arc<RwLock<ReadCache>>
    checkpoint: Arc<CheckpointStore>
    backup:     Arc<BackupManager>
        |
  EncodingLayer（encoding_layer.rs）
    BOM → ASCII 快路径 → chardetng → GetACP() fallback
    safe_read() / safe_write() / 双 hash / CRLF 保留
        |
  Platform Layer
    path.rs：斜杠统一 / collapse_dotdot / 路径逃逸 / \\?\\ 长路径
    terminal.rs：stderr 输出 / ANSI 检测 / crossterm 颜色
```

协议版本（来源：`refer/MCP-DEMO/main.rs`）：
`ProtocolVersion::V_2025_06_18`（rmcp SDK 当前支持的版本，规范最新为 2025-11-25）

---

## 4. 五个核心工具（V5）

来源：`refer/MCP-DEMO/main.rs`（逐字核实参数和 description）

### 4.1 read_file

参数（`ReadFileParams`）：
- `file_path: String` — 相对 workspace，正/反斜杠均可
- `start_line: Option<u32>` — 1-based，默认 1
- `end_line: Option<u32>` — 默认 EOF，最多 2000 行

关键保证：Tab 保留为 `\t`（禁止转为空格，Claude Code Issue #26996）；超 2000 行自动截断。

### 4.2 write

- 未读保护：文件必须先 `read_file`，否则返回 `FileNotRead` 错误
- 保留原始编码（UTF-8/GBK/UTF-16LE）、BOM、行尾符
- 自动 mkdir -p 创建父目录

### 4.3 edit

三级匹配 fallback（来源：`refer/MCP-DEMO/V5-DESIGN.md` §四，`refer/MCP-DEMO/encoding_layer.rs`）：

```
Level 0：精确字节匹配
  1 次命中 → 执行
  0 次 → Level A
  >=2 次（无 replace_all）→ MULTIPLE_MATCHES（含行号+上下文）

Level A：Tab/Space 规范化（try_tab_normalized_match）
  4空格→tab / tab→4空格 / 2空格→tab
  命中 → 执行（附 warning）

Level B：Unicode 混淆检测（find_unicode_confusion）
  U+2019/2018 vs U+0027，U+201C/D vs U+0022
  U+2013/14 vs U+002D，U+00A0/202F vs U+0020
  → STRING_NOT_FOUND（含具体字符差异）
```

额外参数：
- `replace_all: Option<bool>` — 批量替换
- `force: Option<bool>` — 跳过 FILE_MODIFIED 检查（Issue #15887，formatter 竞态）

### 4.4 multi_edit

- 全量预校验 → 原子写入，任一失败文件不变
- 按顺序应用，每次调用只产生一个 Checkpoint 快照

### 4.5 rewind

- 从 SQLite WAL 快照撤销最近 N 步写操作
- Checkpoint DB：`%LOCALAPPDATA%\\agent-editor\\checkpoints\\{session_id}.db`
- session 结束自动清理

---

## 5. 关键子模块

### 5.1 ReadCache（cache.rs，逐字核实）

```rust
pub struct CacheEntry {
    pub raw_hash:      String,  // sha256(原始字节)，Edit 校验主 hash
    pub norm_hash:     String,  // sha256(LF 规范化)，CRLF 容错 hash
    pub encoding_name: String,
    pub line_ending:   LineEnding,
    pub has_non_ascii: bool,
    pub read_at:       DateTime<Utc>,
    pub read_count:    u32,
    pub edit_count:    u32,
}
```

双 hash 解决 CRLF 误报（Issue #13456）：两个 hash 任意一个匹配则视为未修改。

### 5.2 EncodingLayer（encoding_layer.rs，逐字核实）

检测顺序：BOM（置信度 1.0）→ 纯 ASCII 快路径 → chardetng（前 4096 字节）→ GetACP() fallback（Windows 936→GBK, 932→Shift-JIS）。

`safe_write` 含六步（来自真实代码）：
1. 非 ASCII 注入检测（原始文件是纯 ASCII 时）
2. 编码往返校验（`encoding_rs encode`，检测 `had_unmappable`）
3. 添加原始 BOM（UTF-8/UTF-16LE/UTF-16BE）
4. 保留原始换行符（CRLF → `ensure_crlf`）
5. mkdir -p 创建父目录
6. `std::fs::write` 写入

### 5.3 ToolError（error.rs，逐字核实）

所有错误序列化为 JSON（`serde tag = "error"`），Agent 可程序性解析：

```
STRING_NOT_FOUND    — 含 suggestions[]：tab_space_mismatch / unicode_candidate
MULTIPLE_MATCHES    — 含 count + locations[]
FILE_MODIFIED       — 含 read_at（上次读取时间）
FILE_NOT_READ       — 要求先调用 read_file
ENCODING_ROUNDTRIP_FAILED — 含 original_char（U+XXXX 格式）
PATH_ESCAPE         — 路径逃逸 workspace 根目录
```

### 5.4 CheckpointStore（checkpoint.rs，逐字核实）

```sql
CREATE TABLE snapshots (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    file_path    TEXT    NOT NULL,
    content      BLOB    NOT NULL,
    created_at   TEXT    NOT NULL
);
```

实现：`CheckpointStore(Arc<Mutex<Connection>>)`，写操作前自动快照，`rewind(n)` 按逆序恢复最近 n 步。

**⚠️ 写前快照语义（来源：`refer/skills-and-hooks-architecture-advisory.md` §1.3，行业收敛）**

正确的 Checkpoint 语义是「写前快照」而非「写后备份」：

```
✅ 正确：edit / multi_edit 调用前先创建快照，rewind 恢复到「工具执行前」状态
❌ 错误：写入磁盘后才备份，rewind 只能恢复到「上次写入结果」
```

来源对照：Gemini CLI checkpointing 明确要求「before any file-modifying tool is approved by user」时触发 checkpoint，快照内容包含 shadow git commit（完整文件状态）+ 对话 JSON + 待执行的工具调用。这与「预授权快照」模型完全一致。

seeyue-mcp 实现约束：`edit` 和 `multi_edit` 的工具处理函数必须在调用 `safe_write()` 之前调用 `checkpoint.snapshot()`，不得在写入后才快照。

### 5.5 path.rs（逐字核实）

```
resolve(workspace, input)
  → 斜杠统一（\ → /）
  → collapse_dotdot（消除 ../）
  → 路径逃逸检查（is_within workspace）
  → 超 260 字符时添加 \\?\\ 前缀（extended_prefix）
```

---

## 6. V8 扩展工具（M1/M2/M3）

来源：`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md`，`refer/MCP-DEMO/M1/`，`refer/MCP-DEMO/M2/`，`refer/MCP-DEMO/M3/`

| 分组 | 工具 | 实现位置 | 核心依赖 |
|------|------|---------|----------|
| Context 效率 | `file_outline` | `M1/file_outline.rs` | tree-sitter |
| Context 效率 | `read_range` | `M1/read_range.rs` | 复用 read 路径 |
| Context 效率 | `search_workspace` | `M1/search_workspace.rs` | `ignore` crate |
| Context 效率 | `read_compressed` | v8 设计，未在 M1 实现 | tree-sitter |
| 代码导航 | `workspace_tree` | `M3/workspace_tree.rs` | `ignore` crate |
| 代码导航 | `find_definition` | `M3/find_refs.rs` | LSP client |
| 代码导航 | `find_references` | `M3/find_refs.rs` | LSP client |
| 写入验证 | `verify_syntax` | `M2/verify_syntax.rs` | tree-sitter |
| 写入验证 | `preview_edit` | `M2/multi_edit.rs` | diff 模块 |
| Git 集成 | `git_status` | `M3/git_status.rs` | std::process |
| Git 集成 | `git_diff_file` | `M3/git_diff_file.rs` | std::process |
| Windows 专项 | `resolve_path` | v8 设计 | platform/path.rs |
| Windows 专项 | `env_info` | v8 设计 | windows-sys |

三大摩擦点（来源：`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §二）：

```
摩擦点 1：Context 消耗过快
  解法：file_outline（骨架，~200 token）+ read_range（精确行范围）
  对比 read_file 全文（500行文件约 3000 token）

摩擦点 2：定位代码靠字面量搜索
  解法：find_definition / find_references（LSP 语义级跳转）
  git_status 结构化输出（替代 bash git 命令）

摩擦点 3：写入后无法自验证
  解法：verify_syntax（tree-sitter < 5ms）→ 发现问题 → edit → 再验
  preview_edit（dry-run，只计算 diff 不写入）
```

---

## 6.1 V8 新增工具详细规格

来源：`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §四

### 6.1.1 Context 效率组

#### file_outline

**设计动机**：对 500 行 Rust 文件，`read_file` 约消耗 3000 token，`file_outline` 只需约 200 token。

**实现**：`tree-sitter` 解析，`QueryCursor::matches()` 遍历捕获，`Node::kind()` + `Node::start_position()` 提取符号信息。支持 Rust/TypeScript/JavaScript/Python/Go/C/C++，其他语言 fallback 正则匹配。

**输入参数**：

```jsonc
{ "path": "src/auth/jwt.rs", "depth": 1 }  // 0=只顶层, 1=含方法（默认）, 2=全展开
```

**返回格式**：

```jsonc
{
  "path": "src\\auth\\jwt.rs",
  "language": "rust",
  "symbols": [
    { "kind": "struct", "name": "JwtConfig", "line": 12, "visibility": "pub" },
    { "kind": "fn", "name": "create_token", "line": 31,
      "signature": "pub fn create_token(&self, claims: Claims) -> Result<String>",
      "parent": "JwtConfig" }
  ],
  "token_estimate": 187
}
```

`token_estimate`：`signature.len() / 4`（精度 ±20%），让 Agent 在调用前判断 token 预算。

---

#### read_range

**设计动机**：`file_outline` 告知符号行号后，精确读取指定范围，不加载全文。

**实现**：复用 V7 `read_raw_bytes` 路径，编码解码后截取指定行范围。

**输入参数**：

```jsonc
{ "path": "src/auth/jwt.rs", "start": 50, "end": 85 }
```

**返回格式**：

```jsonc
{
  "path": "src\\auth\\jwt.rs",
  "start": 50, "end": 85,
  "total_lines": 247,
  "content": "...",
  "truncated": false
}
```

---

#### search_workspace

**实现**：`ignore::WalkBuilder::standard_filters(true)` 自动遵守 `.gitignore`；`WalkParallel` 多线程并行遍历；`regex` crate 做模式匹配；结果通过 `mpsc::channel` 汇总。

**输入参数**：

```jsonc
{
  "pattern": "expires_in",
  "is_regex": false,
  "file_glob": "src/**/*.rs",
  "context_lines": 2,
  "max_results": 50
}
```

**返回格式**：

```jsonc
{
  "pattern": "expires_in",
  "total_matches": 3,
  "truncated": false,
  "matches": [
    {
      "path": "src\\auth\\jwt.rs",
      "line": 45, "column": 12,
      "content": "    let expires_in = 3600;",
      "context_before": ["    let token_type = \"Bearer\";"],
      "context_after":  ["    let token = create_token(...);"]
    }
  ]
}
```

---

#### read_compressed

**设计动机**：对超 token 预算的文件，跳过样板代码，用占位注释标记跳过位置。

**输入参数**：

```jsonc
{ "path": "src/models/user.rs", "token_budget": 500 }
```

**四级压缩规则**（按 token 压力递增）：

```
Level 1（轻压缩）：跳过连续空行（3+ → 1 行）
Level 2（中压缩）：跳过注释块、#[allow(...)] 属性
Level 3（重压缩）：跳过 use/import 块（→ "// ... N imports omitted"）
Level 4（骨架）：  只保留函数签名 + 前 3 行函数体（→ "// ... N lines"）
```

引擎从 Level 1 递增，直到剩余内容在 `token_budget` 以内或达 Level 4。

---

### 6.1.2 代码导航组

#### workspace_tree

**输入参数**：

```jsonc
{ "depth": 3, "respect_gitignore": true, "show_hidden": false, "min_size_bytes": 0 }
```

**返回格式**：

```jsonc
{
  "root": "C:\\projects\\myapp",
  "tree": [
    { "name": "src", "kind": "dir", "children": [
      { "name": "jwt.rs", "kind": "file", "size": 2156, "language": "rust", "modified_ago": "2h" }
    ]}
  ],
  "summary": { "total_files": 8, "total_dirs": 4, "languages": { "rust": 6, "toml": 2 } }
}
```

`modified_ago` 为人类可读相对时间，让 Agent 快速识别最近修改文件。

---

#### find_definition 和 find_references

**实现架构**：不引入 `tower-lsp`（服务端框架）。LSP 基于 JSON-RPC + `Content-Length: N\r\n\r\n` 头，V8 用 `lsp-types` crate 提供类型定义，sans-io 模式约 200 行代码实现客户端。

**LSP Server 发现策略**（按优先级）：

```
1. 环境变量 AGENT_EDITOR_LSP_CMD
2. 语言自动探测：*.rs → rust-analyzer | *.ts → typescript-language-server
                 *.py → pylsp/pyright  | *.go → gopls
3. 未找到 → LSP_NOT_AVAILABLE 错误 + 安装建议 hint
```

**LSP 会话管理**：LSP server 启动约 200-500ms，`AppState` 中持有 `LspSessionPool`（按语言缓存已启动进程），`Mutex` 保护并发安全。

**find_definition 输入 / 返回**：

```jsonc
// 输入
{ "path": "src/handlers/login.rs", "line": 42, "column": 15 }

// 返回
{
  "symbol": "create_token",
  "definitions": [
    { "path": "src\\auth\\jwt.rs", "line": 31, "column": 12,
      "preview": "    pub fn create_token(&self, claims: Claims) -> Result<String> {" }
  ]
}
```

**find_references 返回**：

```jsonc
{
  "symbol": "create_token",
  "total": 4,
  "references": [
    { "path": "src\\handlers\\login.rs", "line": 42, "preview": "    let token = jwt.create_token(claims)?;" },
    { "path": "src\\handlers\\refresh.rs", "line": 18, "preview": "    let new_token = self.jwt.create_token(claims)?;" }
  ]
}
```

---

### 6.1.3 写入验证组

#### verify_syntax

**实现**：tree-sitter 在 < 5ms 内给出语法合法性答案，无需调用真实编译器（`cargo check` 至少数秒）。通过递归检查 `kind() == "ERROR"` 或 `is_missing()` 定位错误位置。

**同时被 `preview_edit` 和 `multi_edit` 内部调用**：新内容语法无效时，写入磁盘前直接返回 `SYNTAX_ERROR`。

**输入参数**：

```jsonc
{ "path": "src/auth/jwt.rs" }
// 也支持直接传内容（preview 场景）：
// { "content": "fn main() { if true {", "language": "rust" }
```

**返回格式**：

```jsonc
// 成功
{ "valid": true, "language": "rust", "parse_ms": 2 }

// 失败
{
  "valid": false, "language": "rust", "parse_ms": 3,
  "errors": [
    { "line": 45, "column": 1, "kind": "MISSING",
      "message": "expected `}` to close block opened at line 31" }
  ]
}
```

---

#### preview_edit

**实现**：复用 V7 `multi_edit` Phase 1（全量预校验）+ `diff::compute()`，`dry_run=true` 跳过磁盘写入。

**输入 / 返回**：

```jsonc
// 输入
{ "path": "src/auth/jwt.rs", "old_string": "    let expires_in = 3600;", "new_string": "    let expires_in = 86400;" }

// 返回
{ "would_apply": true, "syntax_valid_after": true,
  "diff": { "total_removed": 1, "total_added": 1, "plain": "  Line 45 – 45  ·  1 removed  ·  1 added\n  ..." } }
```

---

### 6.1.4 Git 集成组

#### git_status

**实现**：`std::process::Command::new("git")` 调用 `git status --porcelain=v1 -u`，不引入 `git2`（保持二进制体积）。workspace 不是 git 仓库时返回 `GIT_NOT_REPO`。

**返回格式**：

```jsonc
{
  "branch": "feature/jwt-refresh",
  "modified": ["src\\auth\\jwt.rs"], "added": ["src\\auth\\refresh.rs"],
  "deleted": [], "untracked": ["temp.txt"], "staged": [], "conflicts": [], "clean": false
}
```

---

#### git_diff_file

**设计动机**：让 Agent 核查对某文件的全部累积修改，等同于 `git diff HEAD -- <file>`。

**输入参数**：

```jsonc
{ "path": "src/auth/jwt.rs", "base": "HEAD" }  // 支持 HEAD / HEAD~1 / commit hash / branch name
```

返回与 `edit` 完全一致的 `DiffResult` 结构。

---

### 6.1.5 Windows 专项组

#### resolve_path

**设计动机**：Agent 频繁使用 Linux 风格路径导致工具调用失败。`resolve_path` 接受任意格式路径，返回规范化结果。

处理格式：正斜杠 / 反斜杠 / 混合斜杠 / `..` / `~` / 相对路径 / UNC 路径。

**输入 / 返回**：

```jsonc
// 输入
{ "path": "src/auth/../models/user.rs" }

// 返回
{
  "input":        "src/auth/../models/user.rs",
  "absolute":     "C:\\projects\\myapp\\src\\models\\user.rs",
  "relative":     "src\\models\\user.rs",
  "exists":       true,
  "is_dir":       false,
  "in_workspace": true
}
```

---

#### env_info

**设计动机**：session 开始时用一次调用建立完整环境上下文，`rust_analyzer_available` / `git_available` 让 Agent 在调用前知道哪些工具可用。

**返回格式**：

```jsonc
{
  "os":                      "Windows 11 (10.0.26100)",
  "arch":                    "x86_64",
  "workspace":               "C:\\projects\\myapp",
  "codepage":                936,
  "codepage_name":           "GBK",
  "line_ending":             "CRLF",
  "volume_kind":             "NTFS",
  "disk_free_mb":            48320,
  "rust_analyzer_available": true,
  "git_available":           true,
  "agent_editor_version":    "0.8.0"
}
```

---

## 6.2 V8 底层新增模块

来源：`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §五

### 6.2.1 outline 模块（`src/outline/`）

每种语言对应一个 tree-sitter Query DSL 文件（`.scm`）。Rust 示例：

```scheme
; src/outline/queries/rust.scm
(function_item name: (identifier) @name parameters: (parameters) @params) @fn
(struct_item   name: (type_identifier) @name) @struct
(impl_item     type: (type_identifier) @name) @impl
(const_item    name: (identifier) @name) @const
```

`outline::extract(source, language, depth)` 用 `QueryCursor::matches()` 遍历捕获，构造 `Symbol` 列表。

Token 估算：`estimate_tokens(symbols)` 基于 `signature.len() / 4`（4字符 ≈ 1 token），精度 ±20%。

### 6.2.2 search 模块（`src/search/mod.rs`）

`ignore::WalkBuilder::standard_filters(true)` 启用 `.gitignore`/`.ignore` 自动过滤。`build_parallel()` 返回 `WalkParallel`，多线程并行遍历，`mpsc::channel` 汇总结果。

### 6.2.3 lsp_client 模块（`src/lsp_client/`）

sans-io 模式：`BufRead` 读取，`BufWriter<ChildStdin>` 写入，同步操作在 `spawn_blocking` 内执行。

协议层：发送时序列化 JSON 并加 `Content-Length: N\r\n\r\n` 头；接收时先读 `Content-Length` 头解析长度，再读固定字节数的 JSON body。

`LspSessionPool` 在 `AppState` 中用 `Mutex` 保护，同一时刻只有一个工具调用与 LSP server 通信。

---

## 6.3 V8 完整 Cargo.toml

来源：`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §六

```toml
[package]
name    = "agent-editor"
version = "0.8.0"
edition = "2021"

[dependencies]
# MCP
rmcp     = { version = "0.11", features = ["server", "transport-io"] }
schemars = "1.0"

# 异步运行时
tokio = { version = "1", features = ["full"] }

# 序列化
serde      = { version = "1", features = ["derive"] }
serde_json = "1"

# 编码
chardetng   = "0.1"
encoding_rs = "0.8"

# diff（V7：Histogram）
imara-diff = "0.1"

# 大文件读取（V7：零拷贝）
memmap2 = "0.9"

# 终端渲染
ratatui   = { version = "0.29", features = ["crossterm"] }
crossterm = "0.28"

# 路径规范化
dunce = "1.0"

# Checkpoint
rusqlite = { version = "0.31", features = ["bundled"] }

# V8 新增：tree-sitter（file_outline + verify_syntax）
tree-sitter            = "0.25"
tree-sitter-rust       = "0.23"
tree-sitter-typescript = "0.23"
tree-sitter-python     = "0.23"
tree-sitter-go         = "0.23"
tree-sitter-c          = "0.23"

# V8 新增：目录遍历（search_workspace + workspace_tree）
ignore = "0.4"

# V8 新增：正则（search_workspace）
regex = { version = "1", features = ["perf"] }

# V8 新增：LSP 类型定义
lsp-types = "0.97"

# 工具类
sha2                  = "0.10"
hex                   = "0.4"
chrono                = { version = "0.4", features = ["serde"] }
dirs                  = "5"
unicode-normalization = "0.1"
thiserror             = "2"
anyhow                = "1"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.52", features = [
    "Win32_System_Console",
    "Win32_Globalization",
    "Win32_Foundation",
    "Win32_Storage_FileSystem",
] }

[profile.release]
opt-level     = 3
lto           = "thin"
codegen-units = 1
strip         = true
panic         = "abort"

[build-dependencies]
cc = "1"    # tree-sitter grammar 编译 C 代码
```

**二进制体积预测（release + strip + lto=thin）**：

| 组件 | 估算 |
|------|------|
| V7 基础 | ~4.5MB |
| tree-sitter 核心 | +~0.3MB |
| 5 种语言 grammar | +~1.5MB |
| ignore + regex | +~0.4MB |
| lsp-types + 极简客户端 | +~0.1MB |
| **V8 合计** | **~6.8MB** |

---

## 6.4 V8 完整目录结构

来源：`refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §七

```
agent-editor\
├── Cargo.toml
├── build.rs                          ← tree-sitter grammar C 代码编译
├── src\
│   ├── main.rs                       ← MCP Server + AppState 初始化
│   ├── error.rs                      ← ToolError（含 V8 新增错误码）
│   ├── cache.rs / checkpoint.rs / backup.rs / diff.rs
│   ├── encoding\mod.rs               ← 编码检测 + 往返校验
│   ├── platform\
│   │   ├── path.rs（dunce::canonicalize）
│   │   ├── terminal.rs / io.rs / conin.rs / volume.rs
│   ├── outline\                      ← V8 新增：tree-sitter 符号提取
│   │   ├── mod.rs（extract() + estimate_tokens()）
│   │   ├── language.rs
│   │   └── queries\{rust,typescript,python,go,c}.scm
│   ├── search\mod.rs                 ← V8 新增：ignore + regex 并行搜索
│   ├── lsp_client\                   ← V8 新增：极简 LSP 客户端
│   │   ├── mod.rs（LspClient + LspSessionPool）
│   │   ├── protocol.rs（Content-Length JSON-RPC 读写）
│   │   └── discover.rs（LSP server 自动探测）
│   └── tools\
│       ├── read.rs / write.rs / edit.rs   ← V7 继承
│       ├── file_outline.rs / read_range.rs / search_workspace.rs / read_compressed.rs
│       ├── workspace_tree.rs / find_definition.rs / find_references.rs
│       ├── verify_syntax.rs / preview_edit.rs
│       ├── git_status.rs / git_diff_file.rs
│       └── resolve_path.rs / env_info.rs
└── tests\
    ├── integration.rs（V5 继承，13 项）
    ├── outline.rs / search.rs / lsp.rs / verify_syntax.rs / git.rs  ← V8 新增
```

---

## 7. 与 seeyue-workflows 集成策略

当前项目状态：`seeyue-mcp` 尚未创建，MCP-DEMO 是参考实现。

集成路径（来源：`docs/MCP/08-implementation-plan.md` P0）：

```
seeyue-mcp/src/
  main.rs              — 新建，继承 MCP-DEMO main.rs 结构
                         AppState 扩展：加入 node_bridge + workflow_dir
  tools/
    file_editing.rs    — 直接复用 MCP-DEMO 的 read/write/edit/multi_edit/rewind
    hooks.rs           — 新增：sy_pretool_bash / sy_pretool_write / sy_stop 等
    workflow.rs        — 新增：sy_create_checkpoint / sy_advance_node
  encoding_layer.rs    — 直接复用 MCP-DEMO/encoding_layer.rs
  cache.rs             — 直接复用 MCP-DEMO/cache.rs
  checkpoint.rs        — 直接复用 MCP-DEMO/checkpoint.rs
  backup.rs            — 直接复用 MCP-DEMO/backup.rs
  diff.rs              — 直接复用 MCP-DEMO/diff.rs
  error.rs             — 直接复用 MCP-DEMO/error.rs
  platform/
    path.rs            — 直接复用 MCP-DEMO/path.rs
    terminal.rs        — 直接复用 MCP-DEMO/terminal.rs
  ipc/
    node_bridge.rs     — 新增：JSON 管道调用 Node.js 运行时
```

扩展 `AppState`（在 MCP-DEMO 基础上）：

```rust
#[derive(Clone)]
pub struct AppState {
    // 文件编辑引擎（直接复用 MCP-DEMO）
    pub workspace:   Arc<PathBuf>,
    pub cache:       Arc<RwLock<ReadCache>>,
    pub checkpoint:  Arc<CheckpointStore>,
    pub backup:      Arc<BackupManager>,
    // seeyue-workflows 扩展
    pub node_bridge: Arc<NodeBridge>,   // IPC 调用 Node.js 运行时
    pub workflow_dir: Arc<PathBuf>,     // .ai/workflow/ 目录
}
```
