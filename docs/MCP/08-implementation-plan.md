# 实施路线图与优先级

> 来源：`docs/seeyue-workflows-mcp-integration-windows.md` §第四部分
> 参考：`docs/hooks-architecture-design.md` §1.3，`docs/hooks-improvement-checklis.md`
> 参考：`refer/MCP-DEMO/V5-DESIGN.md`

---

## 1. 总体原则

来源：`docs/hooks-architecture-design.md` §1.3

1. **MCP Server 作为新增层**：不替换现有 `hook-client.cjs`，P0 阶段通过 IPC 调用现有实现
2. **渐进交付**：P0 → P1 → P2 分优先级，每个阶段均可独立验证
3. **向后兼容**：现有 hooks 行为不变，MCP Server 是附加接口
4. **来源可追溯**：所有实现均基于现有源码和文档，不引入未验证的外部设计

---

## 2. P0：MCP Server 基础层（最高优先级）

**目标**：能让 Claude Code / Gemini CLI 通过 MCP 调用 seeyue-workflows 文件编辑能力

### 2.1 交付物

| 组件 | 来源参考 | 说明 |
|------|---------|------|
| `seeyue-mcp` Rust 项目骨架 | `refer/MCP-DEMO/main.rs` | `#[tool_router]` 结构 |
| `AppState` 共享状态 | `refer/MCP-DEMO/lib.rs` | workspace/cache/checkpoint/backup |
| `read_file` 工具 | `refer/MCP-DEMO/main.rs` 行 107-121 | 完整复用 MCP-DEMO 实现 |
| `write` 工具 | `refer/MCP-DEMO/main.rs` 行 123-137 | 完整复用 |
| `edit` 工具 | `refer/MCP-DEMO/main.rs` 行 139-159 | 完整复用 |
| `multi_edit` 工具 | `refer/MCP-DEMO/main.rs` 行 161-175 | 完整复用 |
| `rewind` 工具 | `refer/MCP-DEMO/main.rs` 行 177-185 | 完整复用 |
| encoding_layer | `refer/MCP-DEMO/encoding_layer.rs` | 完整复用 |
| cache | `refer/MCP-DEMO/cache.rs` | 完整复用 |
| checkpoint | `refer/MCP-DEMO/checkpoint.rs` | 完整复用 |
| backup | `refer/MCP-DEMO/backup.rs` | 完整复用 |
| diff | `refer/MCP-DEMO/diff.rs` | 完整复用 |
| platform/path | `refer/MCP-DEMO/V5-DESIGN.md` §三 | Windows 路径规范化 |

### 2.2 客户端配置

```json
// .claude/settings.json（来源：refer/agent-source-code/claude-code-main/）
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
// .gemini/settings.json（来源：refer/agent-source-code/gemini-cli-main/）
{
  "mcpServers": [
    {
      "name": "seeyue-workflows",
      "command": "seeyue-mcp.exe",
      "args": ["--workspace", "${workspaceFolder}"]
    }
  ]
}
```

### 2.3 验证标准

- `seeyue-mcp.exe` 启动时间 < 30ms（来源：`refer/MCP-DEMO/V5-DESIGN.md` §一）
- Claude Code 能通过 `tools/list` 发现 5 个文件编辑工具
- `read_file` → `edit` 完整流程可执行
- Tab 保留、CRLF 保留、编码保留均正常

---

## 3. P1：Hooks 和 Workflow MCP 化

**目标**：Hooks 决策和 Workflow 状态通过 MCP 访问

### 3.1 交付物

| 组件 | 来源参考 | 说明 |
|------|---------|------|
| `sy_pretool_bash` 工具 | `docs/04-hooks-integration.md` §5.1 | IPC → hook-client.cjs |
| `sy_pretool_write` 工具 | `docs/04-hooks-integration.md` §5.2 | IPC → hook-client.cjs |
| `sy_posttool_write` 工具 | `workflow/hooks.spec.yaml` PostToolUse | IPC → hook-client.cjs |
| `sy_stop` 工具 | `docs/04-hooks-integration.md` §5.3 | IPC → hook-client.cjs |
| `sy_create_checkpoint` 工具 | `scripts/runtime/checkpoints.cjs` | IPC → Node.js |
| `sy_advance_node` 工具 | `scripts/runtime/transition-applier.cjs` | IPC → Node.js |
| `workflow://session` Resource | `.ai/workflow/session.yaml` | 直接文件读取 |
| `workflow://task-graph` Resource | `.ai/workflow/task-graph.yaml` | 直接文件读取 |
| `workflow://journal` Resource | `.ai/workflow/journal.jsonl` | 直接文件读取 |
| IPC 桥接（NodeBridge）| `docs/02-architecture.md` §4 | JSON 管道 |

### 3.2 IPC 桥接实现

```rust
// ipc/node_bridge.rs
pub struct NodeBridge {
    child: tokio::process::Child,
    stdin: tokio::process::ChildStdin,
    stdout: BufReader<tokio::process::ChildStdout>,
}

impl NodeBridge {
    pub async fn call(&mut self, method: &str, params: serde_json::Value)
        -> Result<serde_json::Value, BridgeError>
    {
        let req = json!({ "method": method, "params": params, "id": self.next_id() });
        self.stdin.write_all(req.to_string().as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        let mut line = String::new();
        self.stdout.read_line(&mut line).await?;
        let resp: serde_json::Value = serde_json::from_str(&line)?;
        Ok(resp["result"].clone())
    }
}
```

### 3.3 验证标准

- AI 引擎执行危险命令时 `sy_pretool_bash` 正确返回 `block`
- `workflow://session` Resource 返回当前 session.yaml 内容
- 变更通知在 `sy_advance_node` 后正确触发

---

## 4. P2：Skills Prompts 与扩展工具

**目标**：Skills 系统通过 MCP Prompts 暴露；V8 扩展工具集成

### 4.1 交付物

| 组件 | 来源参考 | 说明 |
|------|---------|------|
| Skills → MCP Prompts | `docs/06-skills-as-prompts.md` | prompts/list + prompts/get |
| `file_outline` 工具 | `refer/MCP-DEMO/M1/file_outline.rs` | tree-sitter 符号骨架，~200 token |
| `read_range` 工具 | `refer/MCP-DEMO/M1/read_range.rs` | 按行范围读取，配套 file_outline |
| `search_workspace` 工具 | `refer/MCP-DEMO/M1/search_workspace.rs` | ignore+regex 并行搜索，遵守 .gitignore |
| `read_compressed` 工具 | `refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §4.1 | 四级压缩，跳过样板代码 |
| `workspace_tree` 工具 | `refer/MCP-DEMO/M3/workspace_tree.rs` | 目录树 + 文件元信息 |
| `find_definition` 工具 | `refer/MCP-DEMO/M3/find_refs.rs` | LSP goto-definition，语义级跳转 |
| `find_references` 工具 | `refer/MCP-DEMO/M3/find_refs.rs` | LSP references，全项目引用列表 |
| `verify_syntax` 工具 | `refer/MCP-DEMO/M2/verify_syntax.rs` | tree-sitter 语法校验 < 5ms |
| `preview_edit` 工具 | `refer/MCP-DEMO/M2/multi_edit.rs` | dry-run，只计算 diff 不写入 |
| `git_status` 工具 | `refer/MCP-DEMO/M3/git_status.rs` | 工作区变更结构化摘要 |
| `git_diff_file` 工具 | `refer/MCP-DEMO/M3/git_diff_file.rs` | 单文件相对 HEAD 的完整 diff |
| `resolve_path` 工具 | `refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §4.5 | 任意路径格式 → 绝对路径规范化 |
| `env_info` 工具 | `refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` §4.5 | 环境基础信息（OS/Codepage/LSP可用性）|
| Hooks 四层重构 | `docs/hooks-improvement-checklis.md` §1.1 | registry/planner/runner/aggregator |
| NTFS ACL 保护集成 | `docs/hooks-architecture-design.md` §1.1.2 | apply-ntfs-protection.ps1 |

### 4.2 验证标准

- `prompts/list` 返回全部 22 个 skill（17 顶层 + 子约束）
- `file_outline` 对 TypeScript 文件返回函数/类骨架，`token_estimate` 字段存在
- `search_workspace` 正则搜索结果正确，遵守 `.gitignore`
- `read_compressed` Level 1-4 递增压缩逻辑正确
- `workspace_tree` 返回 `summary.languages` 统计
- `find_definition` / `find_references` LSP server 未安装时返回 `LSP_NOT_AVAILABLE` + hint
- `verify_syntax` 对语法错误文件返回 `ERROR` 节点位置，< 5ms
- `preview_edit` dry-run 返回 `would_apply` + `syntax_valid_after`
- `git_status` 非 git 仓库时返回 `GIT_NOT_REPO`
- `env_info` 返回 `rust_analyzer_available` 和 `git_available` 字段
- `resolve_path` 接受正斜杠路径返回规范化 Windows 绝对路径

---

## 5. 依赖关系图

```
P0（文件编辑引擎）
  │  直接复用 MCP-DEMO 模块
  │
  ▼
P1（Hooks + Workflow）
  │  需要 P0 的 AppState 和 IPC 桥接
  │  需要现有 hook-client.cjs（Node.js 运行时）
  │
  ▼
P2（Skills + 扩展工具）
  需要 P1 的 MCP Server 骨架
  需要 .agents/skills/ 目录结构
  需要 workflow/skills.spec.yaml（已有）
```

---

## 6. 风险与缓解

| 风险 | 可能性 | 缓解措施 |
|------|--------|----------|
| IPC 延迟影响 hooks 响应 | 中 | P0 先独立交付文件编辑，hooks IPC 在 P1 单独验证 |
| rmcp SDK API 变更 | 低 | 锁定 rmcp 版本，参考 MCP-DEMO 已验证的用法 |
| Windows Defender 误报 | 低 | Rust 单文件二进制，提前白名单申请 |
| Node.js bridge 进程崩溃 | 中 | 实现自动重启和降级：IPC 失败时返回 allow（非阻断）|
| CRLF/GBK 编码边界 | 低 | 直接复用 MCP-DEMO encoding_layer，已处理所有 Windows 编码场景 |

---

## 7. 三层协作架构：后续开发核心方向

> 本节说明项目的战略定位转变：从「如何将 MCP 融入 seeyue-workflows」升级为「MCP / Skills / Hooks 三者如何各司其职、协同工作」。

### 7.1 问题背景

P0-P2 阶段的实施计划解决了「MCP 接入」问题，但随着能力层扩展，出现了一个更深层的架构问题：

**三个子系统都在做决策，但职责边界不清晰：**

- `hook-client.cjs` 既做策略决策（verdict），又做状态读取，又做证据记录
- Skills 系统通过 MCP Prompts 暴露，但 Skill 调用链与 Hook 决策链存在重叠
- MCP Tools 提供文件操作能力，但何时用 MCP edit、何时用引擎原生 Write 尚无明确规范

### 7.2 三层职责划分

每层做自己最擅长的事：

```
┌─────────────────────────────────────────────────────────────┐
│  Hooks 层（强制守卫）                                          │
│  职责：阻断决策、阶段门控、预算保护                              │
│  做得好：verdict 强制阻断、exit code 注入、引擎级拦截            │
│  不该做：文件 IO、状态查询、技能加载                             │
├─────────────────────────────────────────────────────────────┤
│  MCP 层（能力执行）                                            │
│  职责：文件编辑、状态暴露、工具调用、跨引擎接口统一               │
│  做得好：编码保留、Checkpoint、Resource 订阅、多引擎统一接口      │
│  不该做：强制阻断引擎行为、承载 workflow 业务逻辑                │
├─────────────────────────────────────────────────────────────┤
│  Skills 层（行为引导）                                         │
│  职责：约束加载、工作流程指导、角色行为规范、上下文注入            │
│  做得好：渐进式加载、persona 隔离、prompt 工程、场景化约束        │
│  不该做：文件 IO、系统命令执行、verdict 决策                    │
└─────────────────────────────────────────────────────────────┘
```

### 7.3 三层协作流程

以「AI 引擎修改一个文件」为例，三层如何配合：

```
1. Skills 层（事前）
   SessionStart → sy-workflow skill 加载约束
   → persona 绑定确认 → 上下文注入完成

2. Hooks 层（守卫）
   PreToolUse:Write → hook-client.cjs
   → 检查文件类别（file-classes.yaml）
   → 检查 persona 写权限
   → 返回 verdict（allow / block / block_with_approval_request）
   ↓ allow
3. MCP 层（执行）
   → read_file（编码检测 + cache）
   → edit（三级匹配 + Checkpoint）
   → verify_syntax（< 5ms 语法校验）
   → PostToolUse:Write → sy_posttool_write（证据记录 → journal）
```

**关键约定**：
- Hooks 只做 **verdict**，不做 IO
- MCP 只做**执行**，不做强制阻断
- Skills 只做**引导**，不做系统调用

### 7.4 当前已实现 vs 待解决

| 层 | 当前状态 | 待解决 |
|----|---------|--------|
| Hooks | 混合职责（决策+IO+记录） | 重构为纯 verdict 决策层（P2 四层分离）|
| MCP | P0 文件编辑已完整 | P1 工具与 hook verdict 的联动协议待定义 |
| Skills | MCP Prompts 暴露方案已设计 | Skill 加载时机与 Hook 触发顺序待规范 |
| 三层联动 | 无统一规范 | **需要新增「三层协作规范」文档（后续开发重点）**|

### 7.5 后续开发重点

以下是 P2 之后的优先级方向，不在当前 P0-P2 范围内，但需提前识别：

1. **三层联动协议文档**：明确 Hook verdict 如何触发 MCP 工具调用，Skill 加载如何影响 Hook 决策上下文
2. **Hook 瘦身重构**：将 `hook-client.cjs` 中的 IO 操作迁移至 MCP，只保留 verdict 决策逻辑
3. **Skill 触发时机规范**：`SessionStart` 时哪些 Skill 必须加载、哪些按需加载，与 MCP `sy_session_start` 协调
4. **统一证据链**：`PostToolUse` 证据目前分散在 hook-client 和 journal，统一到 MCP `sy_posttool_write` 后需规范格式
5. **跨引擎行为一致性测试**：三引擎通过同一 MCP 接口操作后，验证 Hook verdict 和 Skill 约束的行为一致性
6. **Loop Budget 工具层检查**（来源：`refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md` §2.2）：`sy_pretool_bash` 和 `sy_advance_node` 执行前须检查 `loop_budget` 六项指标（max_nodes / max_minutes / max_failures / max_pending_approvals / max_context_utilization / max_rework_cycles），超限时返回 `block` 并附 `budget_exceeded` reason，防止失控自治循环
7. **Crash Recovery 协议**（来源：`refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md` §2.2）：`sy_session_start` 工具应包含 journal 重放检查——读取 `session.yaml` + `journal.jsonl`，发现「有 tool request 无 completion」的孤儿事件时自动补写 `aborted`，按 TDD 状态机规则（red_pending / red_verified / green_verified）决定恢复点，再返回恢复后的完整状态
8. **Gemini Policy Engine 编译**：Gemini adapter 应将 admin-tier TDD 硬约束编译为 `.gemini/policies/v4-workflow.toml`（TOML rules），hook script 只做证据记录；详见 `02-architecture.md` §8.3 G2 插槽

---

## 8. P3+：Agent 开发辅助工具缺口分析

> V8 已解决「写得准」闭环（read → edit → verify_syntax）。P3+ 需要补全「跑得通」「看得全」「改得稳」三个维度。

### 8.1 运行时执行反馈（「跑得通」）

Agent 写完代码后需要知道「跑起来了吗」，`verify_syntax` 只做静态语法，无运行时反馈：

| 工具 | 作用 | 优先级 |
|------|------|--------|
| `run_test` | 执行指定测试，返回结构化 pass/fail/错误信息；TDD 闭环必需，配套 `verify_syntax` | P2+ |
| `lint_file` | 调用 eslint/clippy/ruff，返回结构化诊断；比 `verify_syntax` 语义更深 | P2+ |
| `run_command` | 受控执行命令，返回 stdout/stderr/exit_code 结构化 JSON；需与 Hook 层 verdict 联动 | P3 |

**两阶段输出过滤约束（来源：`refer/agent-source-code/claude-code-security-review-main`）**

`run_test` / `lint_file` 的返回结果应在 MCP 层做两阶段过滤后再返回给 Agent：
1. 硬排除规则（正则）：过滤 DoS/rate-limit/理论性/置信度明显不足的告警 → 直接丢弃
2. 语义摘要：剩余 findings 按 severity 排序，每条附 `confidence_score`

防止 Agent 被低价值告警淹没，造成无效的 approval 中断或重复修复循环。

### 8.2 会话状态感知（「看得全」）

Agent 在长会话中容易失去「我在哪、做了什么」的上下文：

| 工具 | 作用 | 优先级 |
|------|------|--------|
| `session_summary` | 当前 session 结构化摘要：已修改文件列表、活跃节点、预算消耗；配套现有 `workflow://session` Resource | P2+ |
| `diff_since_checkpoint` | 相对上一个 Checkpoint 的全量变更；比 `git_diff_file` 粒度更细（含未 commit 内容）；配套现有 `CheckpointStore` | P2+ |

### 8.3 多文件关联分析（「看得全」）

`find_references` 做了符号级，但缺文件级关联：

| 工具 | 作用 | 优先级 |
|------|------|--------|
| `dependency_graph` | 文件级依赖关系图（谁 import 了谁），评估变更影响范围；需 LSP 会话池支撑 | P3 |
| `symbol_rename_preview` | 重命名符号的全项目 dry-run 预览；配合 LSP rename 协议 | P3 |

### 8.4 跨文件批量操作（「改得稳」）

`multi_edit` 只操作单文件，缺跨文件原子操作：

| 工具 | 作用 | 优先级 |
|------|------|--------|
| `multi_file_edit` | 跨多个文件的原子批量编辑（全量预校验 → 原子写入）；需跨文件 Checkpoint 协调 | P3 |
| `create_file_tree` | 按模板批量创建文件和目录结构（脚手架场景）；替代 Agent 逐文件调用 write | P3 |

### 8.5 外部依赖查询

| 工具 | 作用 | 优先级 |
|------|------|--------|
| `package_info` | 查询 crates.io / npm / pypi 包的最新版本和特性；避免写错版本号 | P3 |
| `type_check` | TypeScript/Python 类型检查（tsc --noEmit / mypy）；比语法校验更严格 | P3 |

---

## 9. 当前 MCP 方案工具全集

> 汇总 P0-P2 已规划的全部工具，作为实施参考基线。

### P0：文件编辑引擎（5 个）

| 工具 | 说明 |
|------|------|
| `read_file` | 全文读取，tab保留，2000行截断，编码检测 |
| `write` | 整文件写入，未读保护，编码/BOM/换行符保留 |
| `edit` | 字符串替换，三级匹配 fallback，Checkpoint 快照 |
| `multi_edit` | 批量编辑，全量预校验 → 原子写入 |
| `rewind` | SQLite WAL 快照撤销最近 N 步 |

### P1：Hooks + Workflow MCP 化（6 个工具 + 3 个 Resource）

| 类型 | 名称 | 说明 |
|------|------|------|
| Tool | `sy_pretool_bash` | 命令执行前 hook，verdict 决策 |
| Tool | `sy_pretool_write` | 文件写入前 hook，TDD/secret/保护文件守卫 |
| Tool | `sy_posttool_write` | 文件写入后 hook，证据记录 |
| Tool | `sy_stop` | Stop hook，checkpoint + resume-frontier 门控 |
| Tool | `sy_create_checkpoint` | 创建 workflow checkpoint |
| Tool | `sy_advance_node` | 推进节点状态 |
| Resource | `workflow://session` | 当前 session.yaml |
| Resource | `workflow://task-graph` | 任务图 task-graph.yaml |
| Resource | `workflow://journal` | 操作日志 journal.jsonl |

### P2：V8 扩展工具（12 个）+ Skills Prompts

| 分组 | 工具 | 说明 |
|------|------|------|
| Context 效率 | `file_outline` | tree-sitter 符号骨架，~200 token |
| Context 效率 | `read_range` | 按行范围读取，配套 file_outline |
| Context 效率 | `search_workspace` | ignore+regex 并行搜索，遵守 .gitignore |
| Context 效率 | `read_compressed` | 四级压缩，跳过样板代码 |
| 代码导航 | `workspace_tree` | 目录树 + 文件元信息 |
| 代码导航 | `find_definition` | LSP goto-definition |
| 代码导航 | `find_references` | LSP references，全项目引用列表 |
| 写入验证 | `verify_syntax` | tree-sitter 语法校验 < 5ms |
| 写入验证 | `preview_edit` | dry-run，只计算 diff 不写入 |
| Git 集成 | `git_status` | 工作区变更结构化摘要 |
| Git 集成 | `git_diff_file` | 单文件相对 HEAD 的完整 diff |
| Windows 专项 | `resolve_path` | 任意路径格式 → 绝对路径规范化 |
| Windows 专项 | `env_info` | 环境信息（OS/Codepage/LSP可用性）|
| Prompts | Skills → MCP Prompts | prompts/list + prompts/get，22 个 skill |

### P3+：待补全工具（预留插槽）

| 分组 | 工具 | 依赖条件 |
|------|------|----------|
| 运行时反馈 | `run_test` | TDD 闭环 |
| 运行时反馈 | `lint_file` | linter 集成 |
| 运行时反馈 | `run_command` | Hook verdict 联动 |
| 会话状态 | `session_summary` | workflow Resource 扩展 |
| 会话状态 | `diff_since_checkpoint` | CheckpointStore 扩展 |
| 关联分析 | `dependency_graph` | LSP 会话池 |
| 关联分析 | `symbol_rename_preview` | LSP rename 协议 |
| 批量操作 | `multi_file_edit` | 跨文件 Checkpoint |
| 批量操作 | `create_file_tree` | 模板引擎 |
| 外部查询 | `package_info` | 网络请求（受控）|
| 外部查询 | `type_check` | tsc/mypy 集成 |

**P0-P2 合计：23 个工具 + 3 个 Resource + 1 个 Prompts 接口**
**P3+ 预留：11 个工具插槽**
