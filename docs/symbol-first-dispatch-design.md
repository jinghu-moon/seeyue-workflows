# MCP Dispatch 层与工具元数据设计

> 基于 `docs/symbol-first-north-star.md` 2.1 / 2.2 / 2.3 / 2.4 节的实施规格文档
> 设计日期：2026-03-19
> 定位：**机器契约**，不是设计原则说明。所有接口、目录、trait 约束必须可直接落地。

---

## 一、问题陈述

当前 seeyue-mcp 存在以下结构性问题：

```
问题 1：协议逻辑与工具逻辑混合
  server/tools_core.rs 中 JSON-RPC 解析、Params 反序列化、工具调用、响应序列化混在一起
  → 新增工具需要同时修改 3 处：tools/xxx.rs + server/tools_core.rs + params/xxx.rs

问题 2：工具无能力元数据
  hooks.spec.yaml 中重复维护工具的 read_only / destructive 等属性
  router 无法通过统一 API 查询工具的安全边界
  → hooks 和 router 各自维护一份不完整的工具描述

问题 3：客户端兼容性处理无固定位置
  Claude vs Codex schema 差异散落在各处
  → 未来接入新客户端时无统一改动点

问题 4：工具暴露策略无机制支撑
  "会话级超集 + 运行时过滤" 原则（symbol-first-north-star.md:2.4）缺少实现约束
  → active/inactive 状态无统一存储
```

本文档提供解决上述四个问题的设计契约。

---

## 二、目录边界契约

### 2.1 目录结构

```
seeyue-mcp/src/
├── tools/                    # 纯业务层（零协议依赖）
│   ├── mod.rs                # pub mod 注册
│   ├── metadata.rs           # ToolMetadata 定义 + 全局注册表
│   └── {tool_name}.rs        # 每个工具：Params + Result + run_*()
│
├── server/                   # 协议适配层（零业务逻辑）
│   ├── mod.rs
│   ├── dispatch.rs           # JSON-RPC → Params → run_*() → Response
│   ├── schema.rs             # tools/list 响应生成（从 metadata 自动推导）
│   └── compat.rs             # 客户端兼容性转换（Claude / Codex schema 差异）
│
├── params/                   # Params 类型定义（可被 tools/ 和 server/ 共用）
│   ├── mod.rs
│   └── {tool_name}.rs
│
└── app_state.rs              # AppState（LspSessionPool、WorkspaceConfig 等）
```

### 2.2 层间依赖规则

```
tools/xxx.rs      →  params/xxx.rs, app_state.rs, error.rs
                     禁止 import：server/*, serde_json（仅用强类型 Params/Result）

server/dispatch.rs →  tools/*, params/*, server/schema.rs, server/compat.rs
                     负责：JSON Value → Params 反序列化，Result → JSON Value 序列化

server/schema.rs  →  tools/metadata.rs
                     负责：生成 MCP tools/list 响应

server/compat.rs  →  （无业务依赖）
                     负责：客户端特定 schema 修复（openai_tools sanitize 等价物）
```

**核心约束**：`tools/` 层不得 import `serde_json::Value`，只处理强类型。协议序列化全部在 `server/` 层。

---

## 三、ToolMetadata 契约

### 3.1 结构定义

```rust
// tools/metadata.rs

/// 工具能力元数据 — 单一数据源，供 hooks / router / schema / dispatch 消费
#[derive(Debug, Clone)]
pub struct ToolMetadata {
    /// 工具唯一名称（与 MCP tools/list 中的 name 一致）
    pub name:                  &'static str,
    /// 人类可读描述（生成 MCP schema 用）
    pub description:           &'static str,
    /// 工具分类，供 hooks gating 使用
    pub category:              ToolCategory,
    /// 是否只读（不修改任何文件或状态）
    pub read_only:             bool,
    /// 是否破坏性操作（删除、覆盖、不可逆）
    pub destructive:           bool,
    /// 是否写 .ai/workflow/ 持久状态（影响 checkpoint 完整性）
    pub mutates_durable_state: bool,
    /// 是否需要交互（调用 sy_approval_request / sy_ask_user 等）
    pub requires_interaction:  bool,
    /// 是否需要 workspace 路径（无 workspace 时直接报错）
    pub requires_workspace:    bool,
    /// 是否在会话初始化时即激活（false = 默认禁用，需 mode 激活）
    pub active_by_default:     bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    FileEdit,       // read_file, write_file, edit, multi_edit
    Git,            // git_log, git_blame
    Nav,            // get_hover_info, go_to_definition, find_references
    Symbol,         // get_symbols_overview, find_symbol, replace_symbol_body
    Workflow,       // sy_advance_node, sy_create_checkpoint, sy_stop
    Interact,       // sy_ask_user, sy_approval_request, sy_input_request
    Session,        // session_summary, compact_journal, search_session
    Exec,           // run_command, run_test, run_script
    Ext,            // package_info, dependency_graph, call_hierarchy
}
```

### 3.2 全局注册表

```rust
// tools/metadata.rs

use std::collections::HashMap;
use std::sync::OnceLock;

static TOOL_REGISTRY: OnceLock<HashMap<&'static str, ToolMetadata>> = OnceLock::new();

pub fn registry() -> &'static HashMap<&'static str, ToolMetadata> {
    TOOL_REGISTRY.get_or_init(|| {
        let mut m = HashMap::new();
        // 每个工具调用 register!() 宏注册
        register_all_tools(&mut m);
        m
    })
}

impl ToolMetadata {
    /// 按名称查询元数据
    pub fn get(name: &str) -> Option<&'static ToolMetadata> {
        registry().get(name)
    }

    /// 查询工具是否在当前 active_tools 集合内
    /// active_tools 使用 HashSet<String>：去重、大小写敏感、顺序无关
    pub fn is_active(name: &str, active_tools: &HashSet<String>) -> bool {
        let meta = match Self::get(name) {
            Some(m) => m,
            None => return false,
        };
        if meta.active_by_default {
            return true;
        }
        active_tools.contains(name)
    }
}
```

### 3.3 工具注册示例

每个 `tools/xxx.rs` 文件末尾通过常量声明元数据：

```rust
// tools/read_file.rs

pub const METADATA: ToolMetadata = ToolMetadata {
    name:                  "read_file",
    description:           "Read file content with optional line range.",
    category:              ToolCategory::FileEdit,
    read_only:             true,
    destructive:           false,
    mutates_durable_state: false,
    requires_interaction:  false,
    requires_workspace:    true,
    active_by_default:     true,
};
```

```rust
// tools/metadata.rs — register_all_tools()
fn register_all_tools(m: &mut HashMap<&'static str, ToolMetadata>) {
    m.insert(read_file::METADATA.name,  read_file::METADATA.clone());
    m.insert(write_file::METADATA.name, write_file::METADATA.clone());
    // ... 所有工具
}
```

**废弃**：`hooks.spec.yaml` 中的 `tool_class` / `read_only` 重复字段，统一从 `ToolMetadata` 读取。

---

## 四、Dispatch 层契约

### 4.1 dispatch.rs 职责边界

```rust
// server/dispatch.rs

/// 单一 dispatch 入口 — JSON-RPC call_tool 处理
pub async fn dispatch_tool(
    name: &str,
    raw_params: serde_json::Value,
    state: &AppState,
) -> Result<serde_json::Value, McpError> {
    // 1. 元数据检查
    let meta = ToolMetadata::get(name)
        .ok_or_else(|| McpError::method_not_found(name))?;

    // 2. active 检查（Exposed Superset + Active Filter）
    // 显式局部块：确保读锁在 await 前释放，不跨异步边界存活
    let is_active = {
        let active = state.active_tools.read().unwrap();
        ToolMetadata::is_active(name, &*active)
    }; // RwLockReadGuard 在此 drop
    if !is_active {
        return Err(McpError::tool_disabled(name));
    }

    // 3. workspace 检查
    if meta.requires_workspace && state.workspace.is_none() {
        return Err(McpError::workspace_required(name));
    }

    // 4. 路由到工具实现（强类型 Params）
    route_tool(name, raw_params, state).await
}

/// 工具路由表 — 唯一需要修改的地方（新增工具时）
async fn route_tool(
    name: &str,
    raw: serde_json::Value,
    state: &AppState,
) -> Result<serde_json::Value, McpError> {
    match name {
        "read_file"  => {
            let p: ReadFileParams = parse(raw)?;
            let r = tools::read_file::run_read_file(p, state)?;
            Ok(serde_json::to_value(r)?)
        }
        "write_file" => {
            let p: WriteFileParams = parse(raw)?;
            let r = tools::write_file::run_write_file(p, state)?;
            Ok(serde_json::to_value(r)?)
        }
        // ... 其余工具
        _ => Err(McpError::method_not_found(name)),
    }
}

fn parse<T: serde::de::DeserializeOwned>(raw: serde_json::Value) -> Result<T, McpError> {
    serde_json::from_value(raw).map_err(|e| McpError::invalid_params(e.to_string()))
}
```

**关键约束**：
- `route_tool` 是唯一需要随新工具修改的位置（O(1) 改动点）
- Params 解析错误统一返回 `McpError::invalid_params`
- 工具 Result 序列化错误统一返回 `McpError::internal_error`

### 4.2 McpError 类型

```rust
// server/dispatch.rs 或 error.rs

#[derive(Debug)]
pub enum McpError {
    MethodNotFound { name: String },
    InvalidParams   { message: String },
    ToolDisabled    { name: String },
    WorkspaceRequired { name: String },
    InternalError   { message: String },
    ToolError(ToolError),  // 透传 tools/ 层错误
}
```

---

## 五、Schema 自动生成契约（tools/list）

### 5.1 schema.rs 职责

```rust
// server/schema.rs

/// 生成 MCP initialize 响应中的 tools 列表
/// 返回会话级超集（包含 active_by_default=false 的工具）
pub fn generate_tools_list() -> Vec<McpToolSchema> {
    registry()  // 自由函数，非 ToolMetadata::registry()
        .values()
        .map(|meta| McpToolSchema {
            name:        meta.name.to_string(),
            description: meta.description.to_string(),
            input_schema: generate_input_schema(meta.name),
            // MCP 2025-03 annotations
            annotations: Some(McpAnnotations {
                read_only_hint:   Some(meta.read_only),
                destructive_hint: Some(meta.destructive),
                // idempotent_hint 按需填充
                idempotent_hint:  None,
            }),
        })
        .collect()
}

/// Params JSON schema — 半自动推导
/// 当前方案：每个 Params struct derive JsonSchema，generate_input_schema 用 match 路由
/// 目标方案（M2 后）：在 register_all_tools() 中把 schema_fn 一并注册到 ToolMetadata，
///   彻底消除 generate_input_schema 中的重复 match arm
///   ToolMetadata 增加字段：schema_fn: fn() -> serde_json::Value
fn generate_input_schema(name: &str) -> serde_json::Value {
    match name {
        "read_file"  => schemars::schema_for!(ReadFileParams).into(),
        "write_file" => schemars::schema_for!(WriteFileParams).into(),
        // ... 每个工具一行，与 route_tool match arm 保持同步
        _ => json!({ "type": "object", "properties": {} }),
    }
}
```

### 5.2 compat.rs 职责

```rust
// server/compat.rs

/// 客户端兼容性转换 — 对应 Serena 的 _sanitize_for_openai_tools
/// 在 tools/list 响应发送前调用
pub fn sanitize_for_client(schema: &mut serde_json::Value, client_type: ClientType) {
    match client_type {
        ClientType::Claude => {
            // Claude 支持完整 JSON Schema draft-07，无需修改
        }
        ClientType::OpenAI => {
            // 不支持 additionalProperties: false 在顶层
            // 不支持 $schema 字段
            // 不支持 const（需转为 enum: [value]）
            remove_unsupported_keywords(schema);
        }
        ClientType::Gemini => {
            // 不支持 anyOf 顶层（需展平为 oneOf 或去除）
            // nullable 字段需转为 type: ["string", "null"] 形式
            flatten_anyof_nullable(schema);
        }
        ClientType::Unknown => {
            // 退化为最保守兼容集：移除所有扩展关键字
            remove_unsupported_keywords(schema);
            flatten_anyof_nullable(schema);
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum ClientType {
    #[default]
    Claude,   // Claude Code / Claude.ai — 完整 JSON Schema draft-07
    OpenAI,   // OpenAI / Codex — 需移除 additionalProperties、$schema、const
    Gemini,   // Google Gemini — 不支持 anyOf 顶层、需展平 nullable
    Unknown,  // 未识别客户端 — 退化为最保守兼容集
}
```

**关键约束**：所有客户端差异处理**只在** `compat.rs` 修改，不得分散到 `tools/` 或 `dispatch.rs`。

---

## 六、会话级超集 + 运行时过滤契约

### 6.1 AppState 扩展

```rust
// app_state.rs

pub struct AppState {
    pub workspace:    Option<PathBuf>,
    pub lsp_pool:     Arc<Mutex<LspSessionPool>>,
    /// 会话级激活工具集合（空集 = 只激活 active_by_default=true 的工具）
    /// 通过 sy_set_active_tools 或 capabilities.yaml 初始化
    /// 语义：HashSet（去重、大小写敏感、顺序无关）
    /// 禁止：Vec 或有序结构，避免重复条目导致状态漂移
    pub active_tools: Arc<RwLock<HashSet<String>>>,
    pub client_type:  ClientType,
    // ...
}
```

### 6.2 工具状态语义

```
Exposed（会话级超集）
  = tools/list 返回的所有工具
  = registry() 全部条目  // 自由函数，定义于 tools/metadata.rs:134
  = 会话开始时固定，不随运行时状态变化

Active（运行时子集）
  = active_by_default=true 的工具
  + active_tools 列表中的工具
  = dispatch_tool() 实际执行的范围

若 agent 调用 Exposed 但非 Active 的工具：
  → McpError::ToolDisabled
  → 不从 tools/list 消失（符合 Serena 2.4 原则）
```

### 6.3 激活策略示例

```yaml
# .ai/workflow/capabilities.yaml
active_tools:
  - read_file
  - write_file
  - sy_get_symbols_overview
  - sy_find_symbol
  # symbol 工具默认非 active，通过此文件按需开启
```

---

## 七、错误模型契约

### 7.1 错误分层

```
ToolError（tools/ 层）
  PathEscape / IoError / FileNotFound / EditFailed
  SyntaxError / UnsupportedLanguage / MissingParameter
  LspNotAvailable / LspError
  → 携带 hint 字段，供 agent 自助恢复

McpError（server/ 层）
  MethodNotFound / InvalidParams / ToolDisabled
  WorkspaceRequired / InternalError
  → 包裹 ToolError（透传）或独立生成
  → 序列化为 JSON-RPC error object
```

### 7.2 错误响应格式

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "error": {
    "code": -32601,
    "message": "Tool 'unknown_tool' not found",
    "data": {
      "tool": "unknown_tool",
      "hint": "Available tools: read_file, write_file, ..."
    }
  }
}
```

**约束**：`hint` 字段必须包含可操作的恢复建议，不得只是错误描述。

---

## 八、迁移路线

### 阶段 M1：元数据层（~半天，无功能变更）

```
1. 新建 tools/metadata.rs，定义 ToolMetadata + ToolCategory
2. 为现有 58 个工具各添加 pub const METADATA
3. 实现 register_all_tools() + registry() + get() + is_active()
4. server/schema.rs 中 tools/list 生成改为从 registry() 读取
   （替换当前手写的工具描述列表）
5. cargo check — 确认编译通过
```

### 阶段 M2：Dispatch 层（~1 天，重构无功能变更）

```
1. 新建 server/dispatch.rs，实现 dispatch_tool() + route_tool()
2. 将 server/tools_core.rs 中的 match arm 迁移到 route_tool()
3. 在 dispatch_tool() 中接入 metadata 检查 + active 检查
4. main.rs 的 call_tool 处理改为调用 dispatch_tool()
5. 运行完整测试套件（cargo test + node tests/）
```

### 阶段 M3：Compat 层（~半天，按需）

```
1. 新建 server/compat.rs，实现 sanitize_for_client()
2. AppState 中增加 client_type 字段
3. tools/list 响应路径接入 compat.rs
```

### 阶段 M4：Active Filter（~半天，能力扩展）

```
1. AppState 增加 active_tools: Arc<RwLock<Vec<String>>>
2. dispatch_tool() 接入 is_active() 检查
3. capabilities.yaml 中支持 active_tools 列表
4. SessionStart hook 读取 capabilities.yaml 初始化 active_tools
```

---

## 九、与 symbol-first-north-star.md 的映射

| symbol-first-north-star.md 节 | 本文档实现位置 |
|----------------------|----------------|
| 2.1 协议适配层独立 | §2 目录边界 + §4 dispatch.rs |
| 2.2 兼容性转换外置 | §5.2 compat.rs |
| 2.3 工具元信息自动生成 | §3 ToolMetadata + §5.1 schema.rs |
| 2.4 会话级超集 + Active Filter | §6 AppState + active_tools |

---

> 文档完成于 2026-03-19。所有接口定义可直接作为实现约束，无需进一步细化即可开始编码。
