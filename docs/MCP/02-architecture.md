# MCP Server 整体架构设计

> 来源：`refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/architecture/index.mdx`
> 来源：`refer/MCP-DEMO/main.rs`，`refer/MCP-DEMO/lib.rs`，`refer/MCP-DEMO/V5-DESIGN.md`
> 来源：`docs/seeyue-workflows-mcp-integration-windows.md` §第三部分
> 来源：`docs/hooks-architecture-design.md` §1.3

---

## 1. MCP 标准架构角色（官方规范）

来源：`specification/2025-11-25/architecture/index.mdx`

```
┌─────────────────────────────────────────────────────┐
│  Host（AI 应用进程）                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐          │
│  │ Client 1 │  │ Client 2 │  │ Client 3 │          │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘          │
└───────┼─────────────┼─────────────┼─────────────────┘
        │             │             │
     stdio        stdio        HTTP/SSE
        │             │             │
  ┌─────▼─────┐ ┌─────▼─────┐ ┌────▼──────┐
  │ Server A  │ │ Server B  │ │ Server C  │
  │ (Local)   │ │ (Local)   │ │ (Remote)  │
  └─────┬─────┘ └─────┬─────┘ └───────────┘
        │             │
   Local Files   Database
```

**设计原则**（来源：`specification/2025-11-25/architecture/index.mdx`）：
1. Server 应极易构建：Host 承担复杂编排，Server 专注单一能力
2. Server 高度可组合：多 Server 无缝组合，模块化设计
3. Server 隔离原则：Server 仅获得必要上下文，无法读取完整对话，无法感知其他 Server
4. 渐进式能力扩展：核心协议最小化，额外能力通过协商启用，保持向后兼容

---

## 2. seeyue-mcp 架构定位

**MCP Server 是 seeyue-workflows 的标准化能力暴露层**，不替换现有 Node.js 运行时，而是在其之上增加跨引擎访问接口。

```
┌────────────────────────────────────────────────────────┐
│              AI Agent 层                                │
│   Claude Code · Gemini CLI · Cursor · VS Code          │
└────────────────────┬───────────────────────────────────┘
                     │  stdio（JSON-RPC 2.0）
                     │  MCP 协议：2025-11-25
                     │  SDK 实际协商版本：2025-06-18（rmcp 限制）
┌────────────────────▼───────────────────────────────────┐
│              seeyue-mcp（Rust MCP Server）              │
│                                                        │
│  Resources    Tools              Prompts               │
│  ─────────    ──────────────     ──────────            │
│  session      sy_pretool_bash    sy-workflow           │
│  task-graph   sy_pretool_write   sy-constraints        │
│  journal      sy_create_chkpt    sy-executing-plans    │
│  sprint       read_file / edit   ...skills...          │
│               sy_advance_node                          │
└──────────┬─────────────────────────────────────────────┘
           │  IPC（JSON over stdin/stdout 或 named pipe）
┌──────────▼─────────────────────────────────────────────┐
│              seeyue-workflows（Node.js 运行时）          │
│  hook-client.cjs  ·  policy.cjs  ·  router.cjs         │
│  transition-applier.cjs  ·  spec-validator.cjs          │
└────────────────────────────────────────────────────────┘
```

---

## 3. 模块划分

来源：`refer/MCP-DEMO/main.rs`，`refer/MCP-DEMO/lib.rs`，`refer/MCP-DEMO/V5-DESIGN.md` §二

```
seeyue-mcp/
├── src/
│   ├── main.rs              # MCP Server 入口，stdio 传输，#[tool_router]
│   ├── lib.rs               # Engine 组件组合（AppState）
│   ├── tools/
│   │   ├── file_editing.rs  # read_file / write / edit / multi_edit / rewind
│   │   ├── hooks.rs         # sy_pretool_bash / sy_pretool_write / sy_stop
│   │   ├── workflow.rs      # sy_create_checkpoint / sy_advance_node / sy_get_state
│   │   └── policy.rs        # sy_evaluate_policy / sy_check_approval
│   ├── resources/
│   │   └── workflow.rs      # Resources：session / task-graph / journal / sprint-status
│   ├── prompts/
│   │   └── skills.rs        # Prompts：skills → MCP Prompts
│   ├── ipc/
│   │   └── node_bridge.rs   # IPC 桥接：调用 Node.js 运行时
│   ├── encoding_layer.rs    # 编码检测（chardetng + encoding_rs）
│   ├── cache.rs             # ReadCache（双 hash 校验）
│   ├── checkpoint.rs        # SQLite WAL 快照
│   ├── backup.rs            # 备份管理
│   ├── diff.rs              # Myers diff + ANSI 渲染
│   ├── error.rs             # 结构化错误（Agent 可解析 JSON）
│   └── platform/
│       ├── path.rs          # Windows 路径规范化
│       └── terminal.rs      # ANSI 终端检测
└── Cargo.toml
```

---

## 4. 共享状态（AppState）

来源：`refer/MCP-DEMO/main.rs` 行 38-44，`refer/MCP-DEMO/lib.rs` 行 23-29

**MCP-DEMO 原始 AppState**：

```rust
#[derive(Clone)]
pub struct AppState {
    pub workspace:  Arc<PathBuf>,
    pub cache:      Arc<RwLock<ReadCache>>,
    pub checkpoint: Arc<CheckpointStore>,
    pub backup:     Arc<BackupManager>,
}
```

**seeyue-mcp 扩展版**：

```rust
#[derive(Clone)]
pub struct AppState {
    // 文件编辑引擎（来自 MCP-DEMO，直接复用）
    pub workspace:    Arc<PathBuf>,
    pub cache:        Arc<RwLock<ReadCache>>,
    pub checkpoint:   Arc<CheckpointStore>,
    pub backup:       Arc<BackupManager>,
    // seeyue-workflows 扩展
    pub node_bridge:  Arc<NodeBridge>,   // IPC 桥接 Node.js 运行时
    pub workflow_dir: Arc<PathBuf>,      // .ai/workflow/ 目录路径
    // ── 三层协作插槽（预留，P2+ 实现）──────────────────────────
    // pub lsp_pool:     Arc<Mutex<LspSessionPool>>, // V8 代码导航
    // pub hook_verdict_cache: Arc<RwLock<VerdictCache>>, // Hook 决策缓存
    // pub skill_registry: Arc<SkillRegistry>,       // Skill 注册表快速查询
    // pub event_bus:    Arc<EventBus>,              // 三层事件总线
    // ────────────────────────────────────────────────────────────
}
```

---

## 5. 工具分组与优先级

来源：`docs/seeyue-workflows-mcp-integration-windows.md` §第三部分

| 优先级 | 分组 | 工具 | 依赖层 |
|--------|------|------|--------|
| P0 | 文件编辑 | `read_file` `write` `edit` `multi_edit` `rewind` | Rust 内部（来自 MCP-DEMO）|
| P0 | Hooks 决策 | `sy_pretool_bash` `sy_pretool_write` `sy_posttool` `sy_stop` | IPC → Node.js |
| P1 | Workflow 状态 | `sy_get_state` `sy_create_checkpoint` `sy_advance_node` | IPC → Node.js |
| P1 | 策略评估 | `sy_evaluate_policy` `sy_check_approval` | IPC → Node.js |
| P2 | Context 效率 | `file_outline` `read_range` `search_workspace` `read_compressed` | Rust 内部（M1）|
| P2 | 代码导航 | `workspace_tree` `find_definition` `find_references` | Rust 内部（M3 + lsp_client）|
| P2 | 写入验证 | `verify_syntax` `preview_edit` | Rust 内部（M2）|
| P2 | Git 集成 | `git_status` `git_diff_file` | Rust 内部（M3）|
| P2 | Windows 专项 | `resolve_path` `env_info` | Rust 内部（platform）|
| **插槽** | **三层协作（预留）** | `sy_skill_trigger` `sy_hook_context` `sy_verdict_notify` | 三层事件总线 |

---

## 6. IPC 桥接策略

MCP Server（Rust）调用现有 Node.js 运行时的方案：

```
方式 A：子进程调用（最简单）
  seeyue-mcp 通过 std::process::Command 调用 node scripts/runtime/xxx.cjs
  适用：低频操作（创建检查点、评估策略）

方式 B：stdin/stdout JSON 管道（推荐）
  启动时 fork 一个持久 Node.js 进程
  通过 newline-delimited JSON 双向通信
  适用：高频操作（hooks 决策）

方式 C：共享文件系统（最保守）
  Rust 直接读写 .ai/workflow/*.yaml
  通过文件锁协调并发
  适用：资源读取（Resources 层）
```

**推荐优先级**：Resources 用 C，Tools 用 B，低频管理操作用 A。

---

## 7. 多引擎互操作

来源：`specification/2025-11-25/architecture/index.mdx`，`docs/seeyue-workflows-mcp-integration-windows.md` §执行摘要

单一 MCP Server 对接所有 AI 引擎（均使用 stdio 传输）：

```json
// .claude/settings.json（Claude Code）
{
  "mcpServers": {
    "seeyue-workflows": {
      "command": "seeyue-mcp.exe",
      "args": ["--workspace", "${workspaceFolder}"]
    }
  }
}
```

```json
// .gemini/settings.json（Gemini CLI）
{
  "mcpServers": {
    "seeyue-workflows": {
      "command": "seeyue-mcp.exe",
      "args": ["--workspace", "${workspaceFolder}"]
    }
  }
}
```

---

## 8. 三层协作插槽设计（后续开发预留）

> 背景说明见 `docs/MCP/08-implementation-plan.md` §7。MCP 层在设计时需为 Hooks 层和 Skills 层预留明确的接入点，避免未来集成时破坏现有结构。

### 8.1 架构插槽位置

```
┌─────────────────────────────────────────────────────────────┐
│  Skills 层                                                   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  插槽 S1：Skill 触发通知                               │   │
│  │  MCP 工具执行前，通知 Skills 层加载相关约束上下文       │   │
│  │  接口预留：sy_skill_trigger(tool_name, context)       │   │
│  └──────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  MCP 层（seeyue-mcp）                                        │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  插槽 M1：Hook verdict 注入点                          │   │
│  │  工具执行前查询 Hook 层决策，结果影响工具是否继续执行    │   │
│  │  当前：MCP 工具独立执行，不感知 Hook verdict            │   │
│  │  预留：AppState.hook_verdict_cache（已在代码注释中标注）│   │
│  └──────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  插槽 M2：执行后事件广播                               │   │
│  │  MCP 工具执行完毕后，通知 Hooks 层记录证据             │   │
│  │  当前：PostToolUse hook 由引擎原生触发                 │   │
│  │  预留：AppState.event_bus（已在代码注释中标注）         │   │
│  └──────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│  Hooks 层                                                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │  插槽 H1：verdict 上下文输入                           │   │
│  │  Hook 做决策时，可查询 MCP 层的当前工具调用上下文       │   │
│  │  接口预留：sy_hook_context(event, tool_context)       │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 8.2 模块目录插槽

当前 `seeyue-mcp/src/` 目录已预留以下位置（当前为空目录或注释占位）：

```
seeyue-mcp/src/
├── tools/
│   ├── file_editing.rs    ← P0 已实现
│   ├── hooks.rs           ← P1 已设计
│   ├── workflow.rs        ← P1 已设计
│   ├── policy.rs          ← P1 已设计
│   └── collab/            ← 插槽：三层协作工具（sy_skill_trigger 等）
│       └── mod.rs         ← 当前空占位，P2+ 实现
├── ipc/
│   ├── node_bridge.rs     ← P1 已设计
│   └── event_bus.rs       ← 插槽：三层事件总线（P2+ 实现）
└── registry/
    └── mod.rs             ← 插槽：Skill 注册表快速查询（P2+ 实现）
```

### 8.3 插槽实现约定

实现插槽时必须遵守以下约束（与 `docs/MCP/08-implementation-plan.md` §7.3 关键约定一致）：

| 插槽 | 实现约束 |
|------|---------|
| S1 Skill 触发通知 | Skills 层只接收通知，不反向调用 MCP 工具 |
| M1 Hook verdict 注入 | 仅作为「建议」，MCP 工具最终执行权不受 Hook 强制约束 |
| M2 执行后事件广播 | 异步广播，不阻塞工具返回 |
| H1 verdict 上下文输入 | Hook 决策逻辑保持纯函数，上下文只读 |
| **G1 Gemini BeforeToolSelection** | **Gemini 专有事件，在 LLM 决定调用哪个工具之前触发；用于动态过滤可用工具列表（如 budget 紧张时禁用高 token 消耗工具）；对应插槽：`tools/collab/gemini_tool_filter.rs`（P2+ 实现）**|
| **G2 Gemini Policy Engine 编译** | **Gemini adapter 应将 admin-tier TDD 硬约束编译为 `.gemini/policies/v4-workflow.toml`，而不是仅安装 hook script；来源：`refer/skills-and-hooks-architecture-advisory.md` §2.2 盲点5** |
