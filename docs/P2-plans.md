# seeyue-mcp P2 实施计划

## 上下文

P0（5 个文件编辑工具）和 P1（6 个 Hook 工具 + 3 个 Resources + PolicyEngine）已完成并上线运行。P2 目标是补全 V8 扩展工具集（12 个工具）和 Skills Prompts 接口，使 seeyue-mcp 从「文件编辑 + 策略守卫」升级为「完整 Agent 开发辅助平台」。

---

## 实施批次（5 个 Batch，按依赖顺序）

### Batch 0：零依赖工具 + Cargo.toml 依赖准备

- `resolve_path` — 复用现有 `platform::path::resolve()`，额外返回诊断信息
- `env_info` — 纯系统调用（OS/codepage/git/LSP 可用性）
- 同时：更新 Cargo.toml 添加全部 P2 依赖

### Batch 1：tree-sitter 基础设施 + 核心工具

- 新建 `src/treesitter/` 模块（`languages.rs` + `symbols.rs`）
- `file_outline` — tree-sitter 符号骨架，~200 token
- `verify_syntax` — tree-sitter 语法检查 < 5ms

### Batch 2：搜索 + 导航

- `read_range` — 行范围读取，依赖 `file_outline` 的符号解析
- `search_workspace` — `ignore`+regex 搜索，使用 `ignore` crate
- `workspace_tree` — 目录树 + 元信息

### Batch 3：高级工具

- `read_compressed` — 4 级压缩读取，依赖 `file_outline`
- `find_definition` / `find_references` — LSP 优先 + grep fallback
- `preview_edit` — dry-run 编辑预览，需从 `edit.rs` 提取纯函数

### Batch 4：Git + Prompts（可与 Batch 2/3 并行）

- `git_status` / `git_diff_file` — git 命令封装
- Skills Prompts（`prompts/list` + `prompts/get`）— SkillRegistry + SKILL.md 读取

---

## 新增文件结构

```plaintext
seeyue-mcp/src/
├── treesitter/          # 新增：tree-sitter 共享基础设施
│   ├── mod.rs
│   ├── languages.rs     # 语言检测 + grammar 映射
│   └── symbols.rs       # node-kind 收集、签名提取、regex fallback
├── git/                 # 新增：Git 命令封装
│   └── mod.rs           # git_output() 共享函数
├── lsp/                 # 新增：LSP 客户端
│   ├── mod.rs           # server 发现 + 超时控制
│   └── protocol.rs      # JSON-RPC initialize/definition/references
├── prompts/             # 新增：MCP Prompts 子系统
│   ├── mod.rs           # prompt_router impl
│   ├── registry.rs      # skills.spec.yaml → SkillRegistry
│   └── substitution.rs  # $ARGUMENTS/$0/$1 替换
└── tools/
    ├── file_outline.rs      # 新增 (Batch 1)
    ├── verify_syntax.rs     # 新增 (Batch 1)
    ├── read_range.rs        # 新增 (Batch 2)
    ├── search_workspace.rs  # 新增 (Batch 2)
    ├── workspace_tree.rs    # 新增 (Batch 2)
    ├── read_compressed.rs   # 新增 (Batch 3)
    ├── find_definition.rs   # 新增 (Batch 3)
    ├── find_references.rs   # 新增 (Batch 3)
    ├── preview_edit.rs      # 新增 (Batch 3)
    ├── git_status.rs        # 新增 (Batch 4)
    ├── git_diff_file.rs     # 新增 (Batch 4)
    ├── resolve_path.rs      # 新增 (Batch 0)
    └── env_info.rs          # 新增 (Batch 0)
```

---

## 关键修改点

### Cargo.toml — 新增依赖

```toml
# P2: tree-sitter
tree-sitter              = "0.22"
tree-sitter-rust         = "0.21"
tree-sitter-python       = "0.21"
tree-sitter-typescript   = "0.21"
tree-sitter-go           = "0.21"

# P2: workspace traversal
walkdir = "2"
ignore  = "0.4"

# P2: LSP server discovery
which = "7"
```

### AppState — 新增字段

```rust
pub struct AppState {
    // P0/P1 现有字段不变
    // P2 新增
    pub skill_registry: Arc<SkillRegistry>,
}
```

### main.rs — 核心变更

1. 新增 `mod treesitter; mod git; mod prompts; mod lsp;`
2. `SeeyueMcpServer` 添加 `prompt_router: PromptRouter<SeeyueMcpServer>` 字段
3. 在 `#[tool_router] impl` 中添加 12 个新 `#[tool]` 方法
4. 新增 `#[prompt_router] impl` 块（`skill_list` + `skill_get`）
5. `ServerHandler` 添加 `#[prompt_handler]`，`enable_prompts()` capability
6. `main()` 中创建 `SkillRegistry`

### error.rs — 新增变体

- `UnsupportedLanguage` / `SyntaxError` — tree-sitter 相关
- `InvalidRegex` — 搜索相关
- `GitNotAvailable` / `GitError` — git 相关
- `LspNotAvailable` / `LspTimeout` — LSP 相关
- `SkillNotFound` — prompts 相关

### edit.rs — 重构

提取 `find_and_replace_in_memory()` 纯函数，供 `preview_edit` 和 `edit` 共享。

---

## 参考实现映射

| 新工具             | 参考源码                                    | 移植策略                                   |
| ------------------ | ------------------------------------------- | ------------------------------------------ |
| `file_outline`     | `refer/MCP-DEMO/M1/file_outline.rs`         | 直接移植，适配 `ToolError` 体系            |
| `read_range`       | `refer/MCP-DEMO/M1/read_range.rs`           | 移植，集成 `file_outline` 符号解析         |
| `search_workspace` | `refer/MCP-DEMO/M1/search_workspace.rs`     | 移植，用 `ignore` crate 替代手写 gitignore |
| `workspace_tree`   | `refer/MCP-DEMO/M3/workspace_tree.rs`       | 移植，用 `ignore` crate                    |
| `find_definition`  | `refer/MCP-DEMO/M3/find_refs.rs`            | 移植 LSP 客户端 + grep fallback            |
| `find_references`  | `refer/MCP-DEMO/M3/find_refs.rs`            | 同上                                       |
| `verify_syntax`    | `refer/MCP-DEMO/M2/verify_syntax.rs`        | 直接移植                                   |
| `preview_edit`     | `refer/MCP-DEMO/M2/multi_edit.rs` (dry_run) | 提取 dry_run 逻辑                          |
| `git_status`       | `refer/MCP-DEMO/M3/git_status.rs`           | 直接移植                                   |
| `git_diff_file`    | `refer/MCP-DEMO/M3/git_diff_file.rs`        | 直接移植                                   |
| `resolve_path`     | 现有 `platform::path::resolve()`            | 封装为独立工具                             |
| `env_info`         | 新实现                                      | 纯系统调用                                 |
| `prompts`          | `rmcp` prompt_stdio.rs 示例                 | 适配 `SkillRegistry`                       |

---

## 验证策略

### 每个 Batch 完成后

1. `cargo build --release` — 编译通过
2. 每个新模块的 `#[cfg(test)]` 单元测试通过
3. 启动 `seeyue-mcp` → `tools/list` 确认新工具出现
4. 手动调用新工具验证基本功能

### 全量完成后

1. `prompts/list` 返回非 `disable_model_invocation` 的技能列表
2. `prompts/get` 返回完整 SKILL.md 内容（`$ARGUMENTS` 已替换）
3. `file_outline` → `read_range` 协同流程验证
4. `verify_syntax` < 5ms 性能验证
5. LSP 不可用时 `find_definition`/`find_references` 降级到 grep fallback

### 关键验收标准（来自 `docs/MCP/08-implementation-plan.md §4.2`）

- `file_outline` 对 TypeScript 文件返回函数/类骨架，`token_estimate` 字段存在
- `search_workspace` 正则搜索正确，遵守 `.gitignore`
- `read_compressed` Level 1-4 递增压缩逻辑正确
- `verify_syntax` 对语法错误文件返回 ERROR 节点位置，< 5ms
- `preview_edit` dry-run 返回 `would_apply` + `syntax_valid_after`
- `git_status` 非 git 仓库时返回 `GIT_NOT_REPO`
- `env_info` 返回 `rust_analyzer_available` 和 `git_available` 字段
- `resolve_path` 接受正斜杠路径返回规范化 Windows 绝对路径

---

## 风险与缓解

| 风险                                   | 概率 | 缓解                                                   |
| -------------------------------------- | ---- | ------------------------------------------------------ |
| tree-sitter C 编译问题                 | 中   | 确保 MSVC 工具链；如编译失败，先跳过 tree-sitter 工具  |
| LSP server 启动延迟                    | 中   | 3 秒超时 → grep fallback；标记 `source` 字段           |
| `prompt_router` + `tool_router` 宏冲突 | 低   | `rmcp` 示例已验证此模式；fallback：独立 wrapper struct |
| P2 工具数量大                          | 中   | 严格按 Batch 顺序，每批独立编译测试                    |

---

**END OF DOCUMENT**