# Workflow 状态作为 MCP Resources

> 来源：`refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/server/resources.mdx`
> 来源：`docs/seeyue-workflows-mcp-integration-windows.md` §第三部分 P2
> 参考：`workflow/runtime.schema.yaml`，`.ai/workflow/` 目录结构

---

## 1. Resources 原语概述（官方规范）

来源：`specification/2025-11-25/server/resources.mdx`

**Resources 是应用驱动（application-driven）的只读数据源**，由 AI 应用决定如何注入上下文。

Server 能力声明：

```json
{
  "capabilities": {
    "resources": {
      "subscribe": true,
      "listChanged": true
    }
  }
}
```

**协议操作**：

| 方法 | 用途 |
|------|------|
| `resources/list` | 列出直接资源（支持分页）|
| `resources/templates/list` | 列出资源模板（RFC 6570 URI 模板）|
| `resources/read` | 读取资源内容（text 或 blob）|
| `resources/subscribe` | 订阅单个资源变更通知 |
| `notifications/resources/updated` | Server → Client：资源内容变更 |
| `notifications/resources/list_changed` | Server → Client：资源列表变更 |

---

## 2. 资源清单

`.ai/workflow/` 下的持久化文件映射为 MCP Resources，供 AI 引擎只读获取工作流上下文。

| MCP URI | 文件路径 | MIME Type | 描述 |
|---------|---------|-----------|------|
| `workflow://session` | `.ai/workflow/session.yaml` | `application/x-yaml` | 当前会话状态（phase/node/loop_budget）|
| `workflow://task-graph` | `.ai/workflow/task-graph.yaml` | `application/x-yaml` | 任务图（phases/nodes/depends_on）|
| `workflow://sprint-status` | `.ai/workflow/sprint-status.yaml` | `application/x-yaml` | Sprint 状态快照 |
| `workflow://journal` | `.ai/workflow/journal.jsonl` | `application/x-ndjson` | 事件日志（只读追加）|
| `workflow://ledger` | `.ai/workflow/ledger.md` | `text/markdown` | 决策账本 |
| `workflow://specs/policy` | `workflow/policy.spec.yaml` | `application/x-yaml` | 策略规范 |
| `workflow://specs/router` | `workflow/router.spec.yaml` | `application/x-yaml` | 路由规范 |

---

## 3. Resources 协议实现

### 3.1 resources/list 响应（含 2025-11-25 新字段 title、icons）

```json
{
  "resources": [
    {
      "uri": "workflow://session",
      "name": "Workflow Session",
      "title": "Current Workflow Session State",
      "description": "Current phase, active node, loop budget and persona",
      "mimeType": "application/x-yaml",
      "annotations": { "audience": ["assistant"], "priority": 1.0 }
    },
    {
      "uri": "workflow://task-graph",
      "name": "Task Graph",
      "title": "Workflow Task Graph",
      "description": "All phases and nodes with status and dependencies",
      "mimeType": "application/x-yaml",
      "annotations": { "audience": ["assistant"], "priority": 0.9 }
    },
    {
      "uri": "workflow://journal",
      "name": "Event Journal",
      "title": "Workflow Event Log",
      "description": "Append-only event log for audit and recovery",
      "mimeType": "application/x-ndjson",
      "annotations": { "audience": ["user", "assistant"], "priority": 0.7 }
    }
  ]
}
```

**注**：`annotations` 字段（audience/priority/lastModified）是 2025-11-25 正式规范的一部分，
帮助 Client 决策哪些资源应注入上下文（来源：`resources.mdx` §Annotations）。

### 3.2 resources/read 实现（Rust）

```rust
// resources/workflow.rs
async fn read_resource(uri: &str, workflow_dir: &Path) -> Result<String, McpError> {
    let file_path = match uri {
        "workflow://session"       => workflow_dir.join("session.yaml"),
        "workflow://task-graph"    => workflow_dir.join("task-graph.yaml"),
        "workflow://sprint-status" => workflow_dir.join("sprint-status.yaml"),
        "workflow://journal"       => workflow_dir.join("journal.jsonl"),
        "workflow://ledger"        => workflow_dir.join("ledger.md"),
        _ => return Err(McpError::invalid_params("Unknown resource URI", None)),
    };
    tokio::fs::read_to_string(&file_path).await
        .map_err(|e| McpError::internal_error(e.to_string(), None))
}
```

错误码（来源：`specification/2025-11-25/server/resources.mdx` §Error Handling）：
- 资源不存在：`-32002`
- 内部错误：`-32603`

### 3.3 变更通知（notifications/resources/updated）

当 workflow 状态变更时，MCP Server 主动推送通知：

```json
{
  "jsonrpc": "2.0",
  "method": "notifications/resources/updated",
  "params": { "uri": "workflow://session" }
}
```

**触发时机**（须 Client 已通过 `resources/subscribe` 订阅）：
- `sy_advance_node` 调用后
- `sy_create_checkpoint` 调用后
- `sy_stop` 完成后

---

## 4. 关键 session.yaml 字段

来源：`workflow/runtime.schema.yaml`

```yaml
# .ai/workflow/session.yaml 关键字段
phase:
  current: "P2"            # 当前活跃阶段
  status: "in_progress"    # pending|in_progress|review|completed|blocked
  parallel_enabled: false  # F2：是否启用阶段并行
  active_ids: null         # F2：并行时的活跃阶段列表

node:
  active_id: "P2-N1"       # 当前活跃节点
  owner_persona: "author"  # 当前负责人 persona
  state: "in_progress"

loop_budget:
  max_nodes: 20
  consumed_nodes: 3
  max_failures: 3
  consumed_failures: 0
  max_concurrent_nodes: 1  # F1：最大并发节点数（默认 1）

approvals:
  pending: false
  active_request: null

recovery:
  restore_pending: false
  restore_reason: null
```

---

## 5. 并发访问控制

来源：`docs/seeyue-workflows-mcp-integration-windows.md` §2.4.2

MCP Server 读取 Resources 时，Node.js 运行时可能正在写入同一文件。控制策略：

```rust
// 读取时使用共享文件锁（Windows SHARE_READ）
let file = OpenOptions::new()
    .read(true)
    .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)  // windows-sys
    .open(&file_path)?;
```

对于 `journal.jsonl`（append-only），无需锁，直接读取当前快照即可。

---

## 6. 资源缓存策略

```rust
// resources/workflow.rs
pub struct ResourceCache {
    pub session_mtime:   Option<SystemTime>,
    pub session_content: Option<String>,
    // ... 其他资源
}

impl ResourceCache {
    pub fn get_or_refresh(&mut self, uri: &str, path: &Path) -> Result<String> {
        let mtime = fs::metadata(path)?.modified()?;
        if self.get_mtime(uri) == Some(mtime) {
            return Ok(self.get_content(uri).unwrap().clone());
        }
        let content = fs::read_to_string(path)?;
        self.update(uri, mtime, content.clone());
        Ok(content)
    }
}
```

缓存失效条件：mtime 变更（与文件编辑引擎 ReadCache 策略一致，来源：`refer/MCP-DEMO/cache.rs`）。
