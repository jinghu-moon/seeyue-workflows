# Serena-Inspired 设计借鉴文档

> 来源：`refer/serena-main` 深度分析（2026-03-19）
> 定位：面向 seeyue-workflows 的 skills + MCP + hooks 下一阶段设计输入
> 性质：**设计约束与模式参考，不是迁移计划**
> 证据引用：所有 Serena 事实均标注源码位置

---

## 一句话总结

> 最值得借鉴的是：内部 contract 稳定、客户端兼容外置、工具能力元数据化、项目状态 durable、行为配置声明式、关键状态变化可观测。

这个项目最值得借鉴的，不是它接了很多语言服务器，而是它把 MCP 适配、工具能力、行为配置、项目状态、长期记忆、可观测性分层做得很清楚。对 skills + hooks + MCP + sy-interact 体系来说，最有价值的是这套分层方法，不是原样抄功能。

---

## 一、层次模型对比

### Serena 的六层

```
能力本体     Tool 基类 + apply() + ToolMarker 标记
协议适配     mcp.py make_mcp_tool → MCPTool（schema sanitize 单独处理）
行为配置     Context YAML（环境画像）+ Mode YAML（行为画像，可叠加）
项目状态     Project + MemoriesManager + .serena/
长期记忆     global/ + project/ memories，正则只读保护
可观测性     Dashboard（Flask）+ ToolUsageStats + MemoryLogHandler
```

### seeyue-workflows 现状

```
能力本体     seeyue-mcp tools/*.rs（58 个工具）
协议适配     main.rs match arm + JSON-RPC handler（与工具逻辑混合）
行为配置     workflow/hooks.spec.yaml + .agents/skills/*.md（分散）
项目状态     .ai/workflow/runtime.json + interactions/ + checkpoints/
长期记忆     memory_write/read（有，但无分层保护）
可观测性     workflow://dashboard resource（轻量）
```

**主要差距**：协议适配与工具逻辑未分离；行为配置无统一声明式格式；工具无能力元数据。

---

## 二、MCP 设计上最值得借鉴的点

### 2.1 协议适配层独立

**Serena 事实**：内部工具先是 `Tool`，再由 `make_mcp_tool` 转成 MCP Tool，协议细节不侵入业务逻辑。
— `refer/serena-main/src/serena/mcp.py:176`

**seeyue-workflows 现状**：工具实现与 JSON-RPC 路由混在 `main.rs`，已有 `src/server/` 拆分但未彻底。

**借鉴方向**：
```
1. tools/xxx.rs       — 纯业务函数，接受强类型 Params，返回 Result<Value, ToolError>
2. server/dispatch.rs — JSON-RPC → Params 解析 → 调用工具 → 序列化响应
```

**声明**：直接借鉴。已有 `src/server/` 目录是正确方向，继续推进。

---

### 2.2 兼容性转换外置

**Serena 事实**：OpenAI/Codex schema 差异放在 `_sanitize_for_openai_tools` 单独处理，不污染内部 contract。
— `refer/serena-main/src/serena/mcp.py:68`

**借鉴方向**：Claude vs Codex schema 差异在 `server/dispatch.rs` 处理，工具实现层不感知客户端类型。

**声明**：直接借鉴。

---

### 2.3 工具元信息自动生成

**Serena 事实**：工具描述、参数 schema、`readOnlyHint`/`destructiveHint` 都能从工具定义推导，减少手写重复配置。
— `refer/serena-main/src/serena/mcp.py:227`

**借鉴方向**：
```
给 seeyue-mcp 每个工具补充能力元数据（Rust attribute 或 sidecar TOML）：

[tool.metadata]
category              = "file_edit"     # file_edit/git/nav/workflow/interact
read_only             = false
destructive           = false
mutates_durable_state = false           # 是否写 .ai/workflow/ 状态文件
requires_interaction  = false
requires_workspace    = true

供 hooks/router/skill-gating 统一消费，消除 hooks.spec.yaml 中重复的工具描述。
```

**声明**：迁移适配（Serena 用 Python 类继承，我们用 Rust 元数据）。

---

### 2.4 会话级稳定工具暴露（Exposed Superset + Active Filter）

**Serena 事实**（原文注释）：
> "Note that the set of exposed tools is fixed for the session, as clients don't react to changes
> in the set of tools, so this is the superset of tools that can be offered during the session.
> If a client should attempt to use a tool that is dynamically disabled, it will receive an error."
> — `refer/serena-main/src/serena/agent.py:545`

- **Exposed（会话级固定超集）**：MCP tools/list 返回的工具集，会话开始时固定
- **Active（运行时过滤）**：工具被动态禁用时客户端调用收到错误，工具不会从列表消失
  — `refer/serena-main/src/serena/agent.py:549`

**seeyue-workflows 现状**：58 个工具全量暴露，hooks 运行时拦截，客户端收到 block 错误。

**借鉴方向（有意延伸，非照搬）**：
```
SessionStart 时根据 workflow 状态（phase/node/tdd_state）计算更小的 Exposed 集合
→ MCP tools/list 返回过滤后集合
→ Hook 从「拦截后报错」变为「根本不出现在列表」

例：TDD 红灯阶段只暴露 read/search/write(仅测试文件)
    TDD 绿灯阶段恢复 write/edit/run_test

注意：这是对 Serena 模型的有意延伸（更严格），不是 Serena 原做法。
```

---

### 2.5 MCP stdout 洁癖

**Serena 事实**：日志严格走 stderr，避免污染 MCP 协议流。这是非常实用的工程纪律。
— `refer/serena-main/src/serena/cli.py:255`

**seeyue-workflows 现状**：已遵循此原则（`stdout` 保持 JSON-RPC 洁净，日志走 stderr）。✅ 已落地

---

## 三、架构上最值得借鉴的点

### 3.1 中心编排器明确

**Serena 事实**：`SerenaAgent` 统一管理 project、tool、context/mode、memory、dashboard、backend，避免状态散在各层。
— `refer/serena-main/src/serena/agent.py:228`

**现状对照**：seeyue-workflows 的 `scripts/runtime/controller.cjs` + `engine-kernel.cjs` 承担类似职责，但工具状态（active_tools）与运行时状态（runtime.json）尚未统一管理。

**借鉴方向**：让 `session capabilities cache` 成为 MCP、hooks、router 的共同读取点，而不是各自独立判断。

---

### 3.2 Context + Mode 声明式配置

**Serena 事实**：
- Context 表示「运行环境画像」（在哪个客户端运行），互斥，会话级固定
- Mode 表示「行为阶段画像」（当前任务阶段），可叠加，AI 可调用 `SwitchModesTool` 切换
  — `refer/serena-main/src/serena/tools/config_tools.py:52`
  — `refer/serena-main/src/serena/config/context_mode.py:29`
- 两者组合后决定工具边界和提示词边界

**借鉴方向**：
```
skill frontmatter 标准化（类比 Serena Context/Mode）：

---
allowed-tools:              # 工具白名单（必填）
  - read
  - search_workspace
required-context: read-only     # 前置状态要求（可选）
mutates-durable-state: false    # 工具元数据消费方（可选）
interaction-mode: text_fallback # elicitation/local_presenter/text_fallback
---
```

**声明**：直接借鉴。与 workflow 节点状态联动是我们的延伸设计，Serena 无此机制。

---

### 3.3 single_project 最小暴露面

**Serena 事实**：某些 context 下会主动压缩工具集合，而不是先全暴露再靠错误拦截。
— `refer/serena-main/src/serena/config/context_mode.py:163`
— `refer/serena-main/src/serena/agent.py:424`

**借鉴方向**：与 2.4 节 active_tools 过滤层结合，SessionStart 时根据 context 计算最小工具集。

---

### 3.4 项目激活是一等公民（Activation Capsule）

**Serena 事实**：激活项目时把语言配置、memory、initial_prompt 打包成 activation message 注入系统提示。
— `refer/serena-main/src/serena/project.py:349`

**借鉴方向**：
```
Activation Capsule（每次 SessionStart）：
  读取 project/architecture/* memories → 注入 session_context
  不再每次重扫文件系统，减少 token 消耗
```

---

### 3.5 Onboarding 是持久化流程，不是聊天记忆

**Serena 事实**：先判断是否已 onboarding，未做则产出项目级 memory，后续直接复用。
— `refer/serena-main/src/serena/tools/workflow_tools.py:10`

**借鉴方向**：
```
sy-init skill（新建）：
  触发条件：.ai/workflow/onboarding_performed 不存在
  执行：workspace_tree → 读 CLAUDE.md/symbol-first-index.md
        → memory_write project/architecture/overview
        → memory_write project/patterns/tools-map
        → 写 .ai/workflow/onboarding_performed（幂等标记）
```

**声明**：直接借鉴。

---

### 3.6 关键 mutation 串行化

**Serena 事实**：用 `TaskExecutor` 单线程队列串行执行关键任务，降低并发状态竞争。
— `refer/serena-main/src/serena/task_executor.py:18`

**借鉴方向**：对 `.ai/workflow/` 写入引入文件锁或串行队列。
优先级：interaction store > checkpoint > journal（已有 append 安全）。

---

### 3.7 可观测性内建

**Serena 事实**：Dashboard 不是附属品，而是日志、配置、memory、任务状态的统一观察面。
— `refer/serena-main/src/serena/dashboard.py:127`

**借鉴方向**：
```
阶段 1（当前）：workflow://dashboard resource（已有）
阶段 2：runtime snapshot + journal + interaction JSON inspector API
阶段 3：轻量 Web dashboard，展示 active_tools / tool_stats / memories / checkpoints
```

---

## 四、工具体系上最值得借鉴的点

### 4.1 工具自描述

**Serena 事实**：工具名从类名派生，参数/说明从 `apply()` 提取，避免「代码一套、文档一套」。
— `refer/serena-main/src/serena/tools/tools_base.py:136`

**借鉴方向**：工具元数据矩阵（见 2.3 节），让 hooks/router 统一消费，消除重复描述。

---

### 4.2 工具能力元数据化（ToolMarker）

**Serena 事实**：`ToolMarkerCanEdit`、`ToolMarkerSymbolicRead`、`ToolMarkerOptional` 等 marker 标记能力维度。
— `refer/serena-main/src/serena/tools/tools_base.py:72`

**借鉴方向**：Rust 侧用 attribute 或 sidecar TOML 实现等价的能力标记（见 2.3 节元数据矩阵）。

---

### 4.3 工具自动注册

**Serena 事实**：`ToolRegistry` 扫描所有 `Tool` 子类自动注册，新增工具不需要到处改入口。
— `refer/serena-main/src/serena/tools/tools_base.py:396`

**seeyue-workflows 现状**：新增工具需手动改 mod.rs / main.rs / lib.rs 三处。

**借鉴方向**：引入 `inventory` crate 或 build.rs 代码生成实现自动注册，消除三处同步问题。

---

### 4.4 跨项目只读查询

**Serena 事实**：`query_project` / `ProjectServer` 允许对外部项目做只读查询，且限制只读工具集。
— `refer/serena-main/src/serena/tools/query_project_tools.py:42`
— `refer/serena-main/src/serena/project_server.py:33`

**借鉴方向**：
```
当前 refer/ 目录适合做成只读查询 context：

workflow/capabilities.yaml 扩展：
  reference_projects:
    - name: serena
      path: refer/serena-main
      read_only: true

sy-query-reference --project serena --symbol "ToolRegistry"
```

**声明**：直接借鉴。避免 agent 每次全仓 grep refer/ 目录。

---

### 4.5 符号优先而不是文件优先

**Serena 事实**：`get_symbols_overview`、`find_symbol` 这种先定位再读的思路，能显著减少 agent 的上下文浪费。
— `refer/serena-main/src/serena/tools/symbol_tools.py:30`

**现状对照**：seeyue-mcp 已有 `file_outline`、`find_definition`、`find_references`，思路一致。✅ 已落地

---

### 4.6 语言支持：Serena 怎么做到支持所有语言

**Serena 事实**：Serena 不自己实现语言解析，它是一个 **LSP 客户端管理器**——为每种语言启动对应的外部 Language Server 进程，通过标准 LSP 协议通信。

```
Serena (Python)
  └─ SolidLanguageServer
       └─ 50+ 个 language server 适配文件（solidlsp/language_servers/）
            每个文件只定义：启动命令 + 文件匹配规则
            剩余全走统一 LSP JSON-RPC 协议
```

**代价**：用户必须自行安装对应的 language server 二进制，Serena 不内置任何解析器。

**现状对照**：seeyue-mcp 当前语言覆盖：

| 层次 | 支持语言 |
|------|----------|
| tree-sitter（完整符号提取） | Rust / Python / TypeScript / TSX / Go（5 种，编译时内嵌） |
| LSP（定义跳转/引用查找） | 以上 5 种 + JavaScript/JSX（依赖用户安装 LS） |
| 语言识别仅（无语义） | C/C++ / Java / Ruby / Swift / Kotlin / C# / Shell |

**扩展方案（按需选择）**：

**方案 A：纯 LSP 扩展（推荐，< 50 行改动）**
```rust
// lsp/mod.rs discover_server() 新增一行即可支持新语言：
"java"   => pick_cmd(language, "jdtls", vec![]),
"c" | "cpp" => pick_cmd(language, "clangd", vec![]),
"ruby"   => pick_cmd(language, "ruby-lsp", vec![]),
"csharp" => pick_cmd(language, "OmniSharp", vec!["-lsp"]),
"kotlin" => pick_cmd(language, "kotlin-language-server", vec![]),
"bash"   => pick_cmd(language, "bash-language-server", vec!["start".into()]),
// ...
```
效果：`find_definition` / `find_references` / `get_hover_info` 对新语言立即可用。
用户需自行安装对应 language server 并加入 PATH（与 Serena 相同策略）；
`pick_cmd` 内部用 `which::which()` 探测，找不到返回友好错误提示安装命令。

**方案 B：tree-sitter grammar 扩展（符号提取增强）**
```toml
# Cargo.toml 新增（按需）
tree-sitter-java   = "0.21"
tree-sitter-c      = "0.21"
tree-sitter-cpp    = "0.21"
```
效果：`file_outline` 对新语言也有符号树，用户零依赖。代价：编译产物增大。

**方案 C：环境变量逃生舱（已有，零代码改动）**
```bash
AGENT_EDITOR_LSP_CMD="clangd" seeyue-mcp  # 临时支持 C++
```
项目已实现 `AGENT_EDITOR_LSP_CMD` 环境变量覆盖（`lsp/mod.rs:275`），单次会话内支持任意语言。

**推荐路线**：
```
立即可做（< 1小时）
  → 方案 A：jdtls / clangd / kotlin-language-server / bash-language-server
    本地 ~/.serena/language_servers/static/ 二进制已就位，只需在 discover_server() 加 match arm

短期（1-2天）
  → 方案 B：tree-sitter 补 Java/C/C++（高频语言，零依赖，file_outline 受益）
  → 方案 D：符号级编辑工具（replace_symbol_body，基于 tree-sitter range，见第八章）

按需
  → Ruby / C# / Swift 视实际项目需求添加
```

**声明**：Serena 的「支持所有语言」本质是把语言支持负担转嫁给用户环境；seeyue-mcp 选择内嵌 tree-sitter 换取零依赖部署。两种取舍各有适用场景。seeyue-mcp 架构是 tree-sitter（符号提取）+ LSP（语义跳转）双保险，语言覆盖不足是配置问题，不是架构问题。

---

## 五、对我们最应该直接借鉴的 6 点

| 优先级 | 借鉴点 | 对应 Serena 设计 | 性质 | 落地产物 |
|--------|--------|-----------------|------|----------|
| P0 | 工具元数据矩阵：`read_only`/`destructive`/`requires_interaction`/`mutates_durable_state` | ToolMarker（`tools_base.py:72`） | 迁移适配 | schema（`workflow/tool-metadata.schema.yaml`）+ Rust attribute |
| P0 | skill frontmatter 标准化：`allowed-tools`/`required-context`/`interaction-mode` | Context/Mode YAML（`context_mode.py:29`） | 直接借鉴 | doc（`docs/skills-frontmatter-spec.md`）+ validator |
| P1 | `server/dispatch` 与 `tools/` 分层：协议解析、兼容转换、响应序列化放 dispatch | `make_mcp_tool`（`mcp.py:176`） | 直接借鉴 | runtime module（`seeyue-mcp/src/server/dispatch.rs`） |
| P1 | active_tools / session capabilities：hooks、router、MCP tools/list 共用同一份会话能力缓存 | Exposed Superset（`agent.py:545`） | 有意延伸 | runtime module（`scripts/runtime/session-capabilities.cjs`）+ hook 集成 |
| P2 | memory 分层与只读保护：`project/architecture/` 设为只读策略保护 | global/project memories | 直接借鉴 | schema（`workflow/memory-policy.yaml`）+ `memory_write` 校验逻辑 |
| P2 | sy-init / onboarding capsule：首次激活生成架构摘要，后续会话复用 | `workflow_tools.py:10` | 直接借鉴 | skill（`.agents/skills/sy-init.md`）+ onboarding 标记文件 |
| P2 | 语言支持扩展：LSP discover_server() 新增 Java/C++/C#/Ruby | solidlsp language_servers/（`lsp/mod.rs:288`） | 迁移适配 | runtime module（`seeyue-mcp/src/lsp/mod.rs` 扩展） |

---

## 六、不建议照搬的部分

| Serena 设计 | 不照搬的原因 |
|------------|------------|
| 完整 LSP 大矩阵（支持超过 30 种语言，见 `symbol-first-index.md:83`） | seeyue-workflows 重点是 workflow/runtime/interaction，不是通用代码语义平台 |
| think_about_* prompt shaping 工具 | 已有 skills + hooks 管行为约束，更适合继续在此层处理 |
| 重型 Web Dashboard 先行 | 先把 inspector contract、journal schema、runtime snapshot 稳住，再决定 UI 形态 |
| 无 hooks 的信任模型（`lessons_learned.md`） | seeyue-workflows 主动选择 hook 守门换来安全性，这是正确的工程取舍 |

---

## 七、分工边界确认（借鉴后不变的部分）

Serena 的分析**强化**了以下已有设计决策：

```
Skills    = 行为引导（对应 Serena context/mode）            ✅ 继续
Hooks     = 硬约束 + 失败策略（Serena 没有，我们主动选择）  ✅ 继续
MCP       = 能力暴露 + durable state                       ✅ 继续
sy-interact = presenter only，不拿决策权                   ✅ 继续
```

Serena 证明了「无 hooks 也能跑」，seeyue-workflows 选择「有 hooks 更安全」——
两者都是合理的工程取舍，方向不同，不是对错之分。

---

---

## 八、稳定语义定位工作流深度分析（深度阅读补充）

> 本章为深度研读 `refer/serena-main/src/solidlsp/ls.py` 和 `symbol_tools.py` 后的补充，
> 记录 Serena 实现「语言无关、行号变化不影响定位」的核心机制。

### 8.1 name_path：稳定定位的基础

Serena 最关键的设计决策：用 **name_path** 而不是行号定位符号。

```
行号定位（fragile）：edit line 42 → 插入代码后行号漂移 → 定位失效
name_path 定位（stable）：edit "MyClass/my_method" → 行号变化无影响 → 永远有效
```

格式规则：
- 普通符号：`ClassName/method_name`
- 重载方法：`ClassName/method_name[0]`、`ClassName/method_name[1]`
- 顶层符号：`function_name`

工作流三步：
```
1. get_symbols_overview(path)   → 符号树（不含 body，廉价）
2. find_symbol("Class/method")  → UnifiedSymbolInformation（含 location.range）
3. replace_symbol_body(...)     → 按 range 做 workspace_edit（原子替换）
```

### 8.2 SolidLanguageServer 三层稳定机制

**层 1：文件缓冲区 + mtime 缓存失效**（`ls.py:LSPFileBuffer`）
```python
if file_modified_date > self._read_file_modified_date:
    self._contents = None   # 磁盘变化 → 缓存失效，重新读
```
文件未改变时不重复读盘；改变后自动失效，LSP 收到 didChange 通知。

**层 2：文档符号缓存（content_hash 驱动）**（`ls.py:1336`）
```python
self._document_symbols_cache[cache_key] = (file_data.content_hash, document_symbols)
```
符号树以文件内容 hash 为 key。相同内容直接命中缓存，不重新请求 LSP。

**层 3：overload_idx 处理同名符号**（`ls.py:1322`）
```python
if total_name_counts[usymbol["name"]] > 1:
    usymbol["overload_idx"] = name_counts[usymbol["name"]]
```
Java 等语言的方法重载自动编号，`name_path` 唯一确定重载版本。

### 8.3 多语言并行启动（LanguageServerManager）

```python
# ls_manager.py:116
for language in languages:
    thread = StartLSThread(language)  # 各语言 LS 并行启动
    thread.start()
```

单 project 可同时运行多个 LS（Rust + TypeScript + Python 并行）。工厂模式：
```
LanguageServerFactory.create_language_server(Language.KOTLIN)
    → Language.KOTLIN.get_ls_class()  （枚举绑定类）
    → KotlinLanguageServer(config, ...)  （子类覆盖 _start_server()）
```

每个 LS 子类只需实现：
- `_start_server()`：启动进程 + 等 ready 事件
- `_create_dependency_provider()`：描述二进制路径/下载规则

其余（`request_definition`、`request_references`、`get_document_symbols`、符号替换）全部在基类实现，子类零重复。

### 8.4 语言特殊性封装示例（ClangdLanguageServer）

Clangd 需要 `compile_commands.json` 才能做跨文件引用。Serena 的封装：
```python
# clangd_language_server.py:35
def _prepare_compile_commands(self):
    # 读 compile_commands.json，相对路径 → 绝对路径
    # 写到 .serena/ 托管目录，不污染项目
```
语言特殊性完全封装在子类，基类接口对所有语言一致。

### 8.5 对 seeyue-mcp 的最高价值借鉴：符号级编辑

seeyue-mcp 现状：`file_outline` 已产出符号树（含 start/end 行），`edit` 工具是行号级文本替换。

最高 ROI 的改进：把「name_path 定位」和「range 替换」串成原子操作：

```rust
// 伪代码：seeyue-mcp 符号级编辑
pub fn replace_symbol_body(
    name_path: &str,    // "MyClass/my_method"
    relative_path: &str,
    new_body: &str,
    workspace: &Path,
) -> Result<(), ToolError> {
    // 1. tree-sitter 解析 → 符号树（已有）
    // 2. 按 name_path 找 (start_line, end_line)  （需实现 name_path 路由）
    // 3. apply_edit_in_memory(range, new_body)    （已有）
}
```

step 1+3 已有，差 step 2（name_path → range 的路由逻辑）。这是纯 Rust 实现，不需要外部 LS 进程，比 Serena 代价更低。

### 8.6 LSP 扩展的正确依赖策略

seeyue-mcp 不能依赖 Serena 的本地目录（`~/.serena/language_servers/static/`），原因：
- 用户可能根本没有安装 Serena
- Serena 目录结构是其内部实现细节，随版本变化
- seeyue-mcp 应是独立可运行的工具

#### Serena 自动下载机制（`_create_dependency_provider()`）

Serena 的完整流程（以 KotlinLanguageServer 为例）：

```
_create_dependency_provider()
    → DependencyProvider(_custom_settings, ls_resources_dir)

create_launch_command()
    1. 检查 ls_specific_settings["ls_path"] → 用户手动指定路径则直接用
    2. 否则调用 _get_or_install_core_dependency()
        → 检测平台 (win-x64 / linux-x64 / osx-arm64 ...)
        → 构造下载 URL（JetBrains CDN + 版本 + 平台后缀）
        → 检查本地缓存目录是否存在（~/.serena/language_servers/KotlinLanguageServer/）
        → 不存在则 FileUtils.download_and_extract_archive(url, dir, "zip")
        → 解压后 chmod +x（Linux/macOS）
        → 返回可执行路径
    3. _create_launch_command(core_path) → [core_path, "--stdio"]
```

关键设计：
- **幂等**：二进制已存在则跳过下载（`if not os.path.exists(kotlin_script)`）
- **版本可覆盖**：`ls_specific_settings["kotlin_lsp_version"]` 覆盖默认版本
- **路径可覆盖**：`ls_specific_settings["ls_path"]` 完全绕过下载
- **平台感知**：`PLATFORM_KOTLIN_SUFFIX` 映射平台→下载包后缀
- **存储位置**：`~/.serena/language_servers/static/<ClassName>/`（用户级，不在项目内）

#### seeyue-mcp 的等价 Rust 实现方案

三层策略，优先级由高到低：

```
策略 1（最高优先）：环境变量覆盖
  AGENT_EDITOR_LSP_CMD=<path>  → 直接用（已有）

策略 2：PATH 探测
  which::which("jdtls") → 找到则用

策略 3：自动下载（按需实现）
  ~/.seeyue-mcp/language_servers/<Language>/  缓存目录
  → 检查缓存 → 不存在则从官方 URL 下载 → 解压 → 返回路径
```

```rust
// 自动下载的核心结构（Rust 等价）
struct LspDependency {
    language:    &'static str,
    binary_name: &'static str,        // 解压后的可执行文件名
    version:     &'static str,        // 默认版本，可被环境变量覆盖
    urls: HashMap<&'static str, &'static str>,  // platform_id → download_url
    install_hint: &'static str,       // PATH 探测失败时的安装提示
}

fn get_or_install(dep: &LspDependency) -> Result<PathBuf, ToolError> {
    // 1. 环境变量覆盖（已有）
    if let Ok(cmd) = std::env::var("AGENT_EDITOR_LSP_CMD") { ... }

    // 2. PATH 探测
    if let Ok(path) = which::which(dep.binary_name) { return Ok(path); }

    // 3. 本地缓存
    let cache_dir = dirs::home_dir()
        .unwrap()
        .join(".seeyue-mcp/language_servers")
        .join(dep.language);
    let bin_path = cache_dir.join(dep.binary_name);
    if bin_path.exists() { return Ok(bin_path); }

    // 4. 下载
    let platform = detect_platform();  // win-x64 / linux-x64 / osx-arm64
    let url = dep.urls.get(platform).ok_or(...)?;
    download_and_extract(url, &cache_dir)?;
    set_executable(&bin_path)?;  // chmod +x on Unix
    Ok(bin_path)
}
```

每种语言的下载配置：

| 语言 | 二进制 | 下载来源 | 备注 |
|------|--------|----------|------|
| C/C++ (clangd) | `clangd` / `clangd.exe` | `github.com/clangd/clangd/releases` | 直接二进制，最简单 |
| Kotlin | `kotlin-lsp.sh` / `kotlin-lsp.cmd` | `download-cdn.jetbrains.com/kotlin-lsp/` | JetBrains CDN |
| Java (jdtls) | JVM wrapper | `https://download.eclipse.org/jdtls/milestones/` | 依赖 JVM，复杂 |
| Bash | npm 包 | `npm install -g bash-language-server` | 依赖 Node.js，不适合自动下载 |
| Go (gopls) | 依赖 `go` 运行时 | `go install golang.org/x/tools/gopls@latest` | 需 Go 环境 |

**下载来源分类**：
- **GitHub Releases**（最可靠）：clangd、taplo 等，有官方二进制，直接 zip 下载解压
- **厂商 CDN**：Kotlin（JetBrains），URL 稳定但需要平台映射
- **依赖运行时**（不适合自动下载）：jdtls 需 JVM、bash-lsp 需 Node.js、gopls 需 Go
- **VS Code Marketplace**：Serena 用于部分 LS，URL 格式不稳定，seeyue-mcp 不推荐此路径

**实现优先级**：
```
当前        → 策略 1+2（已有环境变量 + 补全 discover_server match arm）
短期按需    → 策略 3，优先 clangd（有官方 GitHub releases 二进制，最简单）
后续        → Kotlin（JetBrains CDN）、jdtls（需 JVM，复杂）
bash-lsp    → 依赖 npm，不适合自动下载，保持 PATH 探测 + 安装提示
```

**错误提示设计原则**（PATH 探测失败时）：
- 包含具体安装命令（brew / scoop / apt）
- 包含官方下载链接
- 说明额外前提（如 C++ 需要 `compile_commands.json`）
- 始终提示 `AGENT_EDITOR_LSP_CMD` 逃生舱

```
LSP server 'clangd' not found in PATH.
Install:
  macOS:   brew install llvm
  Windows: scoop install llvm  (or https://github.com/clangd/clangd/releases)
  Linux:   apt install clangd
Note: C/C++ projects need compile_commands.json.
Or set AGENT_EDITOR_LSP_CMD=<path/to/clangd> to bypass PATH lookup.
``` 选择「提示安装」而非「自动下载」是更保守的初始策略，可在后期按需升级到自动下载。

---

### 8.7 针对实际语言栈的覆盖分析与补齐计划

> 目标语言栈：Vue / CSS / JS / TS / Bat / Kotlin / C / C++ / Rust / Shell / Markdown / JSON / TOML / YAML

#### 当前覆盖状态

| 语言 | tree-sitter（符号提取） | LSP（语义跳转） | 状态 |
|------|----------------------|----------------|------|
| Rust | ✅ 内嵌 | ✅ rust-analyzer（PATH） | 完整 |
| TypeScript / JS | ✅ 内嵌 | ✅ typescript-language-server（PATH） | 完整 |
| TSX / JSX | ✅ 内嵌 | ✅ 同上 | 完整 |
| Python | ✅ 内嵌 | ✅ pyright/pylsp（PATH） | 完整 |
| Go | ✅ 内嵌 | ✅ gopls（PATH） | 完整 |
| Vue | ❌ 无 | ❌ 未配置 | 缺失 |
| CSS | ❌ 无 | ❌ 未配置 | 缺失 |
| Kotlin | ❌ 无 | ❌ 未配置 | 缺失 |
| C / C++ | ❌ 无 | ❌ 未配置 | 缺失 |
| Shell / Bash | ❌ 无 | ❌ 未配置 | 缺失 |
| Markdown | ❌ 无 | ❌ 未配置 | 缺失 |
| JSON / TOML / YAML | ❌ 无 | ❌ 未配置 | 结构简单，优先级低 |
| Bat | ❌ 无 | 无专用 LS | 不适用 |

#### 第一层：LSP 扩展（discover_server() 加 match arm）

| 语言 | LS 二进制 | 安装方式 | 自动下载可行？ |
|------|----------|---------|-------------- |
| Vue | `vue-language-server` | `npm i -g @vue/language-server` | 否（npm） |
| C / C++ | `clangd` | brew / scoop / apt | **是**（GitHub Releases） |
| Kotlin | `kotlin-language-server` | 手动下载 | **是**（JetBrains CDN） |
| CSS | `vscode-css-language-server` | `npm i -g vscode-langservers-extracted` | 否（npm） |
| Shell / Bash | `bash-language-server` | `npm i -g bash-language-server` | 否（npm） |
| Markdown | `marksman` | brew / scoop / GitHub Releases | **是**（GitHub Releases） |
| JSON | `vscode-json-language-server` | `npm i -g vscode-langservers-extracted` | 否（npm） |
| TOML | `taplo-lsp` | `cargo install taplo-cli` / GitHub Releases | **是**（GitHub Releases） |
| YAML | `yaml-language-server` | `npm i -g yaml-language-server` | 否（npm） |
| Bat | 无专用 LS | — | 不适用 |

**Vue 特殊说明**：`.vue` 文件内嵌 template / script / style 三种语言，是最复杂的情况。
`vue-language-server`（Volar）内部会再启动 `typescript-language-server` 处理 `<script>` 块，
两个 LS 协同工作。seeyue-mcp 需要在 `discover_server()` 把 `"vue"` 路由到 `vue-language-server`
并处理其特殊初始化参数，不能复用当前的 TS LS 路径。

#### 第二层：tree-sitter grammar 扩展

以下 crate 可直接添加到 `seeyue-mcp/Cargo.toml`：

```toml
tree-sitter-c        = "0.21"   # C
tree-sitter-cpp      = "0.21"   # C++
tree-sitter-kotlin   = "0.3"    # Kotlin
tree-sitter-css      = "0.21"   # CSS
tree-sitter-bash     = "0.21"   # Shell / Bash
tree-sitter-json     = "0.21"   # JSON
tree-sitter-toml     = "0.5"    # TOML（taplo 维护）
tree-sitter-yaml     = "0.6"    # YAML
tree-sitter-markdown = "0.3"    # Markdown（实验性）
```

每种语言在 `treesitter/languages.rs` 各加：
- `TsLanguage` 枚举值
- `ts_language()` match arm
- `grammar_for()` match arm
- `detect_language()` 扩展名映射

Vue 的 tree-sitter 支持较弱（多语言混合文件），file_outline 意义有限，优先用 LSP。

#### 第三层：自动下载优先级

```
可自动下载（有官方二进制）：
  1. clangd      → github.com/clangd/clangd/releases         最简单，优先
  2. marksman    → github.com/artempyanykh/marksman/releases  轻量
  3. taplo-lsp   → github.com/tamasfe/taplo/releases          TOML
  4. Kotlin LS   → download-cdn.jetbrains.com/kotlin-lsp/     JetBrains CDN

保持 PATH 探测 + hint（依赖 npm/运行时）：
  vue-language-server     → npm i -g @vue/language-server
  vscode-css-language-server → npm i -g vscode-langservers-extracted
  bash-language-server    → npm i -g bash-language-server
  yaml-language-server    → npm i -g yaml-language-server
  vscode-json-language-server → npm i -g vscode-langservers-extracted
```

#### 推荐执行顺序

```
第 1 步（< 2 小时，最快见效）
  discover_server() 补 match arm：
    "c" | "cpp"         → clangd
    "kotlin"            → kotlin-language-server
    "css"               → vscode-css-language-server
    "vue"               → vue-language-server
    "bash" | "sh"       → bash-language-server
    "markdown" | "md"   → marksman
    "json"              → vscode-json-language-server
    "toml"              → taplo-lsp
    "yaml" | "yml"      → yaml-language-server
  每个 match arm 带完整三要素 hint（包名 + 平台命令 + 下载链接）

第 2 步（tree-sitter，按需）
  Cargo.toml 加 crate，languages.rs 补枚举值
  优先：C / C++ / Kotlin / CSS / Bash
  次优先：JSON / TOML / YAML / Markdown

第 3 步（自动下载，选择性实现）
  clangd 优先（GitHub Releases，平台映射最简单）
  marksman 次之（同为 GitHub Releases，无运行时依赖）
  Kotlin / taplo 按需
  npm 系列保持 hint，不做自动下载
```

---

> 文档完成于 2026-03-19。分析基于 `refer/serena-main` 源码，结论适用于 seeyue-mcp 当前 Rust 实现。
```