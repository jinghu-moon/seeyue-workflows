# P3 实施计划：运行时执行 + 深度分析 + 三层协作规范

> 前置条件：P0-P2 均已完成。
> 来源：`docs/MCP/08-implementation-plan.md` §7-8
> 编写日期：2026-03-16

---

## 1. P3 范围概览

P3 围绕三个维度补全 Agent 开发辅助能力，同时完成 P2 遗留的三层架构规范化工作：

| 维度 | 关键词 | 核心工具 |
|------|--------|----------|
| 「跑得通」运行时反馈 | TDD 闭环 | `run_command`, `run_test`, `lint_file` |
| 「看得全」深度分析 | 会话感知 + 多文件关联 | `session_summary`, `diff_since_checkpoint`, `dependency_graph`, `symbol_rename_preview` |
| 「改得稳」跨文件操作 | 原子批量 + 脚手架 | `multi_file_edit`, `create_file_tree` |
| 外部依赖查询 | 版本号准确性 | `package_info`, `type_check` |
| 三层架构规范化 | 职责边界清晰 | Hook 瘦身 + Loop Budget + Crash Recovery + 联动协议 |

---

## 2. P3-A：三层协作规范（前置，阻塞后续工具开发）

> 优先级最高。无此规范，后续工具的职责边界继续模糊。
> 来源：`08-implementation-plan.md` §7.5 第 1-4 条

### 2.1 交付物清单

| 交付物 | 目标文件 |
|--------|----------|
| 三层联动协议文档（含 Loop Budget + Crash Recovery + Skill 时机） | `docs/MCP/10-three-layer-protocol.md` |
| Hook 瘦身重构计划（audit → 迁移边界 → 重构） | `docs/MCP/11-hook-slim-plan.md` |
| 统一证据链格式定义 | `docs/MCP/10-three-layer-protocol.md` §evidence-chain |

### 2.2 Loop Budget 工具层检查规范

来源：`refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md` §2.2

`sy_pretool_bash` 和 `sy_advance_node` 执行前须检查六项指标：

| 指标 | 字段 | 超限行为 |
|------|------|----------|
| 节点数上限 | `max_nodes` / `consumed_nodes` | 返回 `block` + `budget_exceeded: nodes` |
| 时间上限 | `max_minutes` | 返回 `block` + `budget_exceeded: time` |
| 失败次数 | `max_failures` / `consumed_failures` | 返回 `block` + `budget_exceeded: failures` |
| 待审批上限 | `max_pending_approvals` | 返回 `block` + `budget_exceeded: approvals` |
| 上下文利用率 | `max_context_utilization` | 返回 `block` + `budget_exceeded: context` |
| 返工周期 | `max_rework_cycles` | 返回 `block` + `budget_exceeded: rework` |

### 2.3 Crash Recovery 协议规范

来源：`refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md` §2.2

`sy_session_start` 工具应包含 journal 重放检查：

1. 读取 `session.yaml` + `journal.jsonl`
2. 发现「有 tool request 无 completion」的孤儿事件 → 自动补写 `aborted`
3. 按 TDD 状态机规则决定恢复点：
   - `red_pending` → 保持 red_pending（测试未通过）
   - `red_verified` → 可继续 green_pending
   - `green_verified` → 可继续 refactor_pending
4. 返回恢复后的完整状态

---

## 3. P3-B：运行时执行反馈（「跑得通」）

> 来源：`08-implementation-plan.md` §8.1

### 3.1 `run_command`

受控执行 shell 命令，返回结构化 stdout/stderr/exit_code JSON。

**约束**：
- 调用前必须经 `sy_pretool_bash` verdict（hook 联动）
- 工作目录锁定在 workspace 内，禁止 cd 逃逸
- 超时：默认 30s，最大 300s
- stdout/stderr 截断阈值：10000 字符，超出附 `truncated: true`

**参数**：
```rust
struct RunCommandParams {
    command:     String,
    timeout_ms:  Option<u64>,
    working_dir: Option<String>,
    env:         Option<HashMap<String, String>>,
}
```

**返回**：
```json
{ "exit_code": 0, "stdout": "...", "stderr": "...",
  "truncated": false, "duration_ms": 123, "command_class": "build" }
```

### 3.2 `run_test`

TDD 闭环必需工具，配套 `verify_syntax`。

**语言自动检测**（基于 workspace 根文件）：

| 检测文件 | Runner |
|----------|--------|
| `Cargo.toml` | `cargo test [filter]` |
| `package.json` (jest) | `npx jest [filter]` |
| `package.json` (vitest) | `npx vitest run [filter]` |
| `pyproject.toml` / `setup.py` | `pytest [filter]` |

**两阶段输出过滤**（来源：`refer/agent-source-code/claude-code-security-review-main`）：
1. 硬排除正则：过滤 DoS/rate-limit/理论性/置信度不足告警 → 直接丢弃
2. 语义摘要：剩余 findings 按 severity 排序，每条附 `confidence_score`

**参数**：
```rust
struct RunTestParams {
    filter:     Option<String>,
    language:   Option<String>,
    timeout_ms: Option<u64>,  // 默认 60000ms
}
```

### 3.3 `lint_file`

语义深度比 `verify_syntax` 更深，返回结构化诊断。

**适配器**：

| 语言 | 命令 |
|------|------|
| Rust | `cargo clippy --message-format json` |
| TypeScript | `npx eslint --format json [path]` |
| Python | `ruff check --output-format json [path]` |

**参数**：
```rust
struct LintFileParams {
    path:   String,
    linter: Option<String>,  // 强制指定，默认自动检测
    fix:    Option<bool>,    // 默认 false
}
```

---

## 4. P3-C：会话状态感知（「看得全」）

> 来源：`08-implementation-plan.md` §8.2

### 4.1 `session_summary`

当前 session 结构化摘要，配套现有 `workflow://session` Resource。

**参数**：无（读 `session.yaml` + `task-graph.yaml`）

**返回**：
```json
{
  "run_id":         "...",
  "phase":          "execute",
  "active_node":    { "id": "n-03", "title": "...", "tdd_state": "green_pending" },
  "modified_files": ["src/tools/run_command.rs"],
  "loop_budget":    { "consumed_nodes": 3, "max_nodes": 20, "consumed_failures": 0 },
  "pending_approvals": 0,
  "checkpoint_count": 5
}
```

### 4.2 `diff_since_checkpoint`

相对上一个 Checkpoint 的全量变更，比 `git_diff_file` 粒度更细（含未 commit 内容）。

配套现有 `CheckpointStore`（SQLite WAL 快照）。

**参数**：
```rust
struct DiffSinceCheckpointParams {
    label:  Option<String>,  // 指定 checkpoint label，默认最近一个
    paths:  Option<Vec<String>>,  // 过滤文件路径
}
```

---

## 5. P3-D：多文件关联分析（「看得全」）

> 来源：`08-implementation-plan.md` §8.3

### 5.1 `dependency_graph`

文件级依赖关系图，评估变更影响范围。需 LSP 会话池支撑（现有 `LspSessionPool`）。

**参数**：
```rust
struct DependencyGraphParams {
    path:  String,          // 起点文件
    depth: Option<usize>,   // 传播深度，默认 2
    direction: Option<String>,  // "imports" | "imported_by" | "both"
}
```

**返回**：节点为文件路径，边为 import 关系，含 `impact_count` 字段。

### 5.2 `symbol_rename_preview`

全项目重命名符号 dry-run 预览，配合 LSP rename 协议。

**参数**：
```rust
struct SymbolRenamePreviewParams {
    path:     String,
    line:     usize,
    column:   usize,
    new_name: String,
}
```

**返回**：受影响的文件列表 + 每文件变更行数，不执行实际写入。

---

## 6. P3-E：跨文件批量操作（「改得稳」）

> 来源：`08-implementation-plan.md` §8.4

### 6.1 `multi_file_edit`

跨多文件原子批量编辑：全量预校验 → 原子写入。

**约束**：
- 所有文件的所有 edits 先全部校验，任意失败 → 全部回滚
- 每次调用生成一个跨文件 Checkpoint 组（复用 `CheckpointStore`）
- 单次调用最大文件数：20

**参数**：
```rust
struct MultiFileEditParams {
    edits: Vec<FileEditSet>,  // { file_path, edits: Vec<SingleEdit> }
    verify_syntax: Option<bool>,  // 写入后逐文件语法校验，默认 true
}
```

### 6.2 `create_file_tree`

按模板批量创建文件和目录结构，替代 Agent 逐文件调用 write。

**参数**：
```rust
struct CreateFileTreeParams {
    base_path: String,
    tree: Vec<FileNode>,  // { path, content: Option<String>, template: Option<String> }
    overwrite: Option<bool>,  // 默认 false（存在则跳过）
}
```

---

## 7. P3-F：外部依赖查询

> 来源：`08-implementation-plan.md` §8.5

### 7.1 `package_info`

查询 crates.io / npm / pypi 包的最新版本和特性，避免 Agent 写错版本号。

**实现**：HTTP GET（需 Windows 网络权限），结果缓存 TTL=1h（内存）。

**参数**：
```rust
struct PackageInfoParams {
    name:     String,
    registry: Option<String>,  // "crates" | "npm" | "pypi"，默认自动检测
    version:  Option<String>,  // 指定版本查询特性，默认 latest
}
```

### 7.2 `type_check`

TypeScript/Python 类型检查，比语法校验更严格。

| 语言 | 命令 |
|------|------|
| TypeScript | `npx tsc --noEmit` |
| Python | `mypy [path] --json-output` |

---

## 8. 依赖关系图

```
P3-A（三层架构规范）← 前置，阻塞所有 P3 工具开发
  │
  ▼
P3-B（运行时反馈）       P3-C（会话感知）
  run_command              session_summary
  run_test                 diff_since_checkpoint
  lint_file
  │                        │
  └──────────┬─────────────┘
             ▼
         P3-D（多文件关联）
           dependency_graph
           symbol_rename_preview
             │
             ▼
         P3-E（跨文件批量）
           multi_file_edit
           create_file_tree
             │
             ▼
         P3-F（外部依赖）← 独立，无上游依赖
           package_info
           type_check
```

---

## 9. 验证标准

| 工具 | 验收条件 |
|------|----------|
| `run_command` | 超时正确截断；workspace 外路径被拒绝；exit_code 非零时 stderr 完整返回 |
| `run_test` | Rust/TS/Python 三种 runner 自动检测正确；两阶段过滤后噪音 < 原始 20% |
| `lint_file` | Rust clippy 输出 JSON 结构化；fix=true 时写入文件并返回修改行数 |
| `session_summary` | session.yaml 不存在时返回 `SESSION_NOT_FOUND`；loop_budget 字段完整 |
| `diff_since_checkpoint` | 无 checkpoint 时返回 `NO_CHECKPOINT`；diff 输出包含行号 |
| `dependency_graph` | LSP 不可用时返回 `LSP_NOT_AVAILABLE` + grep fallback 结果 |
| `symbol_rename_preview` | LSP 不可用时返回 `LSP_NOT_AVAILABLE`；结果包含 `affected_files_count` |
| `multi_file_edit` | 任意 edit 失败时所有文件均未修改；`verify_syntax=true` 时语法错误被捕获 |
| `create_file_tree` | 父目录自动创建；`overwrite=false` 时已存在文件被跳过并记录 |
| `package_info` | 网络不可用时返回 `NETWORK_ERROR`；缓存命中时 `cached: true` |
| `type_check` | tsc/mypy 未安装时返回 `TOOL_NOT_FOUND` + 安装提示 |

---

## 10. 文件变更清单（预计）

### 新增文件

```
seeyue-mcp/src/tools/run_command.rs
seeyue-mcp/src/tools/run_test.rs
seeyue-mcp/src/tools/lint_file.rs
seeyue-mcp/src/tools/session_summary.rs
seeyue-mcp/src/tools/diff_since_checkpoint.rs
seeyue-mcp/src/tools/dependency_graph.rs
seeyue-mcp/src/tools/symbol_rename_preview.rs
seeyue-mcp/src/tools/multi_file_edit.rs
seeyue-mcp/src/tools/create_file_tree.rs
seeyue-mcp/src/tools/package_info.rs
seeyue-mcp/src/tools/type_check.rs
docs/MCP/10-three-layer-protocol.md  ← P3-A
docs/MCP/11-hook-slim-plan.md        ← P3-A
```

### 修改文件

```
seeyue-mcp/src/main.rs            ← 注册 11 个新工具 + sy_session_start
seeyue-mcp/src/tools/mod.rs      ← 导出新模块
seeyue-mcp/Cargo.toml            ← 新增 reqwest（package_info HTTP 客户端）
seeyue-mcp/src/tools/hooks.rs    ← loop_budget 六项指标检查（P3-A）
docs/MCP/00-index.md              ← 补充 P3 文档索引
```

---

## 11. Cargo.toml 新增依赖

```toml
# P3: HTTP client for package_info
reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
```

> 注：`run_command` / `run_test` / `lint_file` / `type_check` 均调用本地系统命令（`tokio::process::Command`），无额外依赖。
