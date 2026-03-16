# MCP 协议基础与通信规范

> 来源：`refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/`
> 来源：`refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/basic/`
> 来源：`refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/server/`
> 参考：`refer/MCP-DEMO/V5-DESIGN.md`

---

## 1. 协议定位

MCP（Model Context Protocol）是开放标准，定义 AI 应用与外部数据源/工具之间的双向通信协议。

**核心特征**（来源：`specification/2025-11-25/index.mdx`）：
- 消息格式：JSON-RPC 2.0，**必须（MUST）** UTF-8 编码
- 传输层：**stdio**（本地进程）/ **Streamable HTTP**（远程，2025-03-26 取代旧版 HTTP+SSE）
- 官方规范**不支持 WebSocket 传输**
- 当前最新稳定规范版本：**2025-11-25**（历史版本：2024-11-05、2025-03-26、2025-06-18）
- 协议性质：有状态（stateful），Streamable HTTP 支持无状态子集
- Schema 唯一真实来源：`schema/2025-11-25/schema.ts`（TypeScript）

---

## 2. 架构角色

来源：`specification/2025-11-25/architecture/index.mdx`

```
┌─────────────────────────────────────┐
│  Host（AI 应用进程）                  │
│  ├─ Client 1 ◄─────► Server A       │
│  ├─ Client 2 ◄─────► Server B       │
│  └─ Client 3 ◄─────► Server C       │
└─────────────────────────────────────┘
```

| 角色 | 职责 |
|------|------|
| **Host** | AI 应用进程（如 Claude Code、Cursor），创建和管理多个 Client，执行安全策略，协调 LLM 集成 |
| **Client** | 由 Host 创建，与单个 Server 维持 1:1 有状态连接，处理协议协商和能力交换 |
| **Server** | 暴露 Tools/Resources/Prompts，可本地（stdio）或远程运行（Streamable HTTP）|

---

## 3. 生命周期

来源：`specification/2025-11-25/basic/lifecycle.mdx`

三阶段：**Initialization → Operation → Shutdown**

```
Client                          Server
  │                               │
  │── initialize request ────────►│  (必须是首次交互)
  │◄─ initialize response ────────│
  │── initialized notification ──►│  (Client 就绪信号)
  │                               │
  │    [Operation Phase]          │
  │◄───────── 双向消息 ────────────│
  │                               │
  │── Disconnect ────────────────►│
```

### 3.1 initialize 请求（2025-11-25）

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "2025-11-25",
    "capabilities": {
      "roots":       { "listChanged": true },
      "sampling":    {},
      "elicitation": { "form": {}, "url": {} },
      "tasks":       { "requests": { "elicitation": { "create": {} } } }
    },
    "clientInfo": {
      "name": "ExampleClient",
      "version": "1.0.0",
      "description": "An example MCP client",
      "icons": [{ "src": "https://example.com/icon.png", "mimeType": "image/png", "sizes": ["48x48"] }]
    }
  }
}
```

### 3.2 initialize 响应（2025-11-25）

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "2025-11-25",
    "capabilities": {
      "logging":   {},
      "prompts":   { "listChanged": true },
      "resources": { "subscribe": true, "listChanged": true },
      "tools":     { "listChanged": true },
      "tasks":     { "list": {}, "cancel": {} }
    },
    "serverInfo": {
      "name": "ExampleServer",
      "version": "1.0.0",
      "description": "An example MCP server"
    },
    "instructions": "Optional instructions for the client"
  }
}
```

### 3.3 版本协商规则

- Client 发送自己支持的最新版本
- Server 响应相同版本（支持）或自己支持的最新版本
- Client 不支持 Server 返回版本时**应当（SHOULD）**断开连接
- HTTP 传输时后续请求**必须（MUST）**携带 `MCP-Protocol-Version` HTTP 头

### 3.4 能力协商清单（2025-11-25）

| 类型 | 能力 | 说明 |
|------|------|------|
| Client | `roots` | 提供文件系统根目录列表 |
| Client | `sampling` | 支持 Server 发起的 LLM 采样请求 |
| Client | `elicitation` | 支持 Server 向用户请求额外信息（form + url 两种模式）|
| Client | `tasks` | 支持 Task 增强的客户端请求（实验性）|
| Server | `tools` | 暴露可调用工具（可选 `listChanged`）|
| Server | `resources` | 暴露可读资源（可选 `subscribe`、`listChanged`）|
| Server | `prompts` | 暴露提示模板（可选 `listChanged`）|
| Server | `logging` | 发送结构化日志消息 |
| Server | `completions` | 支持参数自动补全 |
| Server | `tasks` | 支持 Task 增强的服务端请求（实验性）|

---

## 4. 服务端三大核心原语

来源：`specification/2025-11-25/server/tools.mdx`，`resources.mdx`，`prompts.mdx`

| 原语 | 控制方 | 发现 | 调用 | 说明 |
|------|--------|------|------|------|
| **Tools** | 模型（model-controlled） | `tools/list` | `tools/call` | 可执行函数，LLM 自主决策调用 |
| **Resources** | 应用（application-driven） | `resources/list`、`resources/templates/list` | `resources/read` | 只读数据源 |
| **Prompts** | 用户（user-controlled） | `prompts/list` | `prompts/get` | 预定义模板，用户显式触发 |

### 4.1 Tools（工具）

工具字段（2025-11-25 新增 `title`、`icons`、`outputSchema`、`execution.taskSupport`）：

```json
{
  "name": "get_weather",
  "title": "Weather Information Provider",
  "description": "Get current weather information for a location",
  "inputSchema": {
    "type": "object",
    "properties": { "location": { "type": "string" } },
    "required": ["location"]
  },
  "outputSchema": { "type": "object", "properties": { "temperature": { "type": "number" } } },
  "execution": { "taskSupport": "optional" }
}
```

**工具名称规范（2025-11-25 新增）**：1-128 字符，大小写敏感，仅允许 ASCII 字母/数字/`_`/`-`/`.`，禁止空格。

**工具结果内容类型**：`text`、`image`、`audio`、`resource_link`、`resource`（嵌入）

**结构化输出**：`structuredContent` 字段返回 JSON 对象；提供 `outputSchema` 时 Server 必须验证。

**错误处理（两级）**：
- Protocol Errors（`-32602` 等）：未知工具、格式错误
- Tool Execution Errors（`isError: true`）：工具内部错误，允许模型自我纠正（2025-11-25 明确规范）

**seeyue-workflows 映射**：
- 文件编辑：`read_file` / `write` / `edit` / `multi_edit` / `rewind`
- Hooks 决策：`sy_pretool_bash` / `sy_pretool_write` / `sy_posttool_write` / `sy_stop`
- Workflow 状态：`sy_create_checkpoint` / `sy_advance_node` / `sy_get_state`

### 4.2 Resources（资源）

资源字段（2025-11-25 新增 `title`、`icons`、`annotations.lastModified`）：

```json
{
  "uri": "file:///project/src/main.rs",
  "name": "main.rs",
  "title": "Rust Software Application Main File",
  "mimeType": "text/x-rust",
  "annotations": { "audience": ["user", "assistant"], "priority": 0.8, "lastModified": "2025-01-12T15:00:58Z" }
}
```

资源模板（RFC 6570 URI 模板）：

```json
{ "uriTemplate": "workflow://{resource}", "name": "Workflow State", "mimeType": "application/x-yaml" }
```

**标准 URI Scheme**：`https://`、`file://`、`git://`，可自定义（须符合 RFC3986）

**seeyue-workflows 映射**：

| MCP URI | 文件 | MIME Type |
|---------|------|-----------|
| `workflow://session` | `.ai/workflow/session.yaml` | `application/x-yaml` |
| `workflow://task-graph` | `.ai/workflow/task-graph.yaml` | `application/x-yaml` |
| `workflow://journal` | `.ai/workflow/journal.jsonl` | `application/x-ndjson` |
| `workflow://sprint-status` | `.ai/workflow/sprint-status.yaml` | `application/x-yaml` |
| `workflow://specs/policy` | `workflow/policy.spec.yaml` | `application/x-yaml` |

### 4.3 Prompts（提示模板）

Prompt 字段（2025-11-25 新增 `title`、`icons`）：

```json
{
  "name": "code_review",
  "title": "Request Code Review",
  "description": "Asks the LLM to analyze code quality",
  "arguments": [
    { "name": "code",     "description": "The code to review",    "required": true },
    { "name": "language", "description": "Programming language", "required": false }
  ]
}
```

`prompts/get` 响应返回 `messages` 数组，每条消息含 `role`（user/assistant）和 `content`（text/image/audio/resource）。

**seeyue-workflows 映射**：Skills → MCP Prompts（详见 `06-skills-as-prompts.md`）

---

## 5. JSON-RPC 2.0 消息格式

来源：`specification/2025-11-25/basic/index.mdx`

### 5.1 请求

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": { "name": "read_file", "arguments": { "file_path": "src/main.rs" } }
}
```
- ID 必须为 string 或 integer，**禁止（MUST NOT）**为 null
- 同一 session 内 ID **禁止（MUST NOT）**重复

### 5.2 成功响应

```json
{ "jsonrpc": "2.0", "id": 1, "result": { "content": [{ "type": "text", "text": "..." }] } }
```

### 5.3 错误响应

```json
{ "jsonrpc": "2.0", "id": 1, "error": { "code": -32602, "message": "Invalid params", "data": {} } }
```

### 5.4 通知（Notification）

无 `id` 字段，接收方**禁止（MUST NOT）**发送响应：

```json
{ "jsonrpc": "2.0", "method": "notifications/resources/updated", "params": { "uri": "workflow://session" } }
```

### 5.5 核心方法清单

| 方法 | 方向 | 说明 |
|------|------|------|
| `initialize` | C→S | 握手协商能力 |
| `notifications/initialized` | C→S | Client 就绪通知 |
| `tools/list` | C→S | 列出可用工具（支持分页）|
| `tools/call` | C→S | 调用工具 |
| `resources/list` | C→S | 列出直接资源（支持分页）|
| `resources/templates/list` | C→S | 列出资源模板 |
| `resources/read` | C→S | 读取资源内容 |
| `resources/subscribe` | C→S | 订阅资源变更 |
| `prompts/list` | C→S | 列出提示模板（支持分页）|
| `prompts/get` | C→S | 获取提示内容 |
| `notifications/resources/updated` | S→C | 资源内容变更 |
| `notifications/resources/list_changed` | S→C | 资源列表变更 |
| `notifications/tools/list_changed` | S→C | 工具列表变更 |
| `notifications/prompts/list_changed` | S→C | 提示列表变更 |

---

## 6. 传输层

来源：`specification/2025-11-25/basic/transports.mdx`

### 6.1 stdio 传输（seeyue-mcp 采用）

- 客户端将 MCP Server 作为子进程启动
- 服务端从 `stdin` 读取 JSON-RPC 消息，向 `stdout` 写入响应
- 消息以换行符分隔，**禁止（MUST NOT）**嵌入换行
- 服务端可向 `stderr` 写任何日志（含 info/debug），客户端**不应（SHOULD NOT）**将 stderr 视为错误条件
- 客户端**应当（SHOULD）**尽可能支持 stdio

### 6.2 Streamable HTTP 传输（远程服务器）

引入版本：2025-03-26（取代 2024-11-05 的 HTTP+SSE）。

- 服务端暴露单一 MCP endpoint，同时支持 POST 和 GET
- 客户端通过 HTTP POST 发送 JSON-RPC 消息，Accept 头**必须（MUST）**包含 `application/json` 和 `text/event-stream`
- 服务端可返回 `application/json`（单次）或 `text/event-stream`（SSE 流）
- **Session 管理**：服务端在 `InitializeResult` HTTP 响应中可通过 `MCP-Session-Id` 头分配 session；后续请求**必须（MUST）**携带该头
- **断线恢复**：客户端通过 HTTP GET + `Last-Event-ID` 头恢复 SSE 流（无论流由 POST 还是 GET 建立，恢复始终通过 GET）
- **安全要求**：
  - 服务端**必须（MUST）**验证 `Origin` 头，无效时返回 HTTP 403
  - 本地运行时**应当（SHOULD）**仅绑定 `127.0.0.1`
  - HTTP 请求**必须（MUST）**携带 `MCP-Protocol-Version` 头

### 6.3 为何选择 stdio

来源：`refer/MCP-DEMO/V5-DESIGN.md` §一

```
Node.js MCP Server  启动：300-800ms  内存：~80MB   Windows Defender：每次扫描
Rust MCP Server     启动：< 30ms    内存：< 8MB   Windows Defender：一次扫描后缓存
```

---

## 7. 2025-11-25 相对 2025-06-18 新增内容

来源：`specification/2025-11-25/changelog.mdx`

| 类别 | 变更 |
|------|------|
| 授权 | OpenID Connect Discovery 1.0 支持；OAuth 2.0 Protected Resource Metadata（RFC 9728 对齐）|
| 图标 | Tools/Resources/Prompts/Implementation 新增 `icons` 字段 |
| 工具名 | 规范化命名指南（1-128 字符，字符集限制）|
| 采样 | Sampling 新增 `tools`/`toolChoice` 参数支持工具调用 |
| Elicitation | `ElicitResult` 和 `EnumSchema` 标准化；新增 URL mode 触发 |
| Tasks | 实验性支持：可轮询的持久执行包装器（`basic/utilities/tasks`）|
| Schema | JSON Schema 2020-12 作为默认方言（无 `$schema` 字段时）|
| 错误处理 | 输入验证错误作为 Tool Execution Error 返回（允许模型自我纠正）|

---

## 8. MCP SDK 选型（seeyue-mcp）

来源：`refer/MCP-DEMO/main.rs`

```toml
# Cargo.toml — rmcp 官方 Rust SDK
[dependencies]
rmcp = { git = "https://github.com/modelcontextprotocol/rust-sdk" }
```

```rust
// main.rs — 宏驱动工具路由
use rmcp::{
    tool, tool_handler, tool_router,
    transport::stdio,
    handler::server::router::tool::ToolRouter,
    model::*,
};

// rmcp 当前协商版本：ProtocolVersion::V_2025_06_18
// （SDK 限制；规范最新为 2025-11-25）
```

`rmcp` 核心宏：
- `#[tool_router]`：自动生成工具路由 dispatch
- `#[tool]`：从文档注释生成 description，从参数类型生成 inputSchema
- `Parameters<T>`：类型安全参数反序列化（`schemars` 驱动）
