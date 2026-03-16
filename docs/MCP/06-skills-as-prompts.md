# Skills 系统作为 MCP Prompts

> 来源：`refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/server/prompts.mdx`
> 来源：`docs/skills-architecture.md`，`workflow/skills.spec.yaml`
> 参考：`docs/skills-catalog.md`，`docs/skills-implementation-guide.md`

---

## 1. Prompts 原语概述（官方规范）

来源：`specification/2025-11-25/server/prompts.mdx`

**Prompts 是用户驱动（user-controlled）的预定义模板**，需用户显式触发（如 slash 命令）。

Server 能力声明：

```json
{ "capabilities": { "prompts": { "listChanged": true } } }
```

**协议操作**：

| 方法 | 用途 |
|------|------|
| `prompts/list` | 列出可用提示模板（支持分页）|
| `prompts/get` | 获取指定提示的完整内容（含参数替换）|
| `notifications/prompts/list_changed` | Server → Client：提示列表变更 |

---

## 2. 映射原理

seeyue-workflows 的 Skills 系统与 MCP Prompts 天然对应：

| Skills 概念 | MCP Prompts 概念 |
|------------|------------------|
| Skill ID（`sy-workflow`）| Prompt `name` |
| Skill `title` | Prompt `title`（2025-11-25 新增）|
| Skill summary | Prompt `description` |
| Skill `$ARGUMENTS` | Prompt `arguments` |
| Skill SKILL.md 内容 | Prompt 返回的 `messages[].content.text` |
| Skill `tier: core` | 常驻加载（always available）|
| Skill `tier: auxiliary` | 按需加载（on-demand）|
| `disable-model-invocation: true` | 不在 `prompts/list` 中暴露 |

来源：`workflow/skills.spec.yaml` skill_tiers 定义

---

## 3. Prompts 清单

### 3.1 Core Skills（常驻，来源：`workflow/skills.spec.yaml` tier: core）

| Prompt Name | 描述 |
|-------------|------|
| `sy-constraints` | 基线工作流约束 |
| `sy-executing-plans` | 执行阶段约束 |
| `sy-workflow` | 工作流路由统一入口 |
| `sy-verification-before-completion` | 完成前验证 |
| `sy-workflow-constraints` | 工作流约束 |

### 3.2 Auxiliary Skills（按需，来源：`workflow/skills.spec.yaml` tier: auxiliary）

| Prompt Name | 描述 |
|-------------|------|
| `sy-changelog` | 变更日志生成 |
| `sy-code-insight` | 代码洞察分析 |
| `sy-debug` | 结构化调试 |
| `sy-design` | 技术设计 |
| `sy-development-workflow` | 开发流程 |
| `sy-doc-sync` | 文档同步 |
| `sy-git-commit` | Git 提交规范 |
| `sy-ideation` | 需求分析 |
| `sy-receiving-code-review` | 处理代码审查反馈 |
| `sy-requesting-code-review` | 请求代码审查 |
| `sy-worktree` | 工作树管理 |
| `sy-writing-plans` | 执行计划编写 |

---

## 4. 协议消息格式

### 4.1 prompts/list 响应（含 2025-11-25 新字段 title、icons）

**⚠️ Progressive Disclosure 实现约束（来源：`refer/skills-and-hooks-architecture-advisory.md` §1.2，Codex 验证模式）**

`prompts/list` **只返回 metadata stub**（name + title + description + arguments schema），绝不内联 SKILL.md 完整内容：

```
✅ 正确：prompts/list 返回 stub → prompts/get 才返回完整 SKILL.md 内容
❌ 错误：prompts/list 内联全部 skill 文本 → 大型 skill 注册表耗尽 context
```

Codex 生产验证结论：skill metadata 与 instructions 必须分离，metadata 始终编译进上下文，instructions 仅在 `prompts/get` 调用时加载。22 个 skill 全量内联约需 50-100K token，必然超出 context 预算。

`workflow/skills.spec.yaml` 中每个 skill 的 `metadata` 字段（name/description/arguments）编译进 `prompts/list` 响应，`instructions` 字段（完整 SKILL.md 内容）仅在 `prompts/get` 时返回。

```json
{
  "prompts": [
    {
      "name": "sy-workflow",
      "title": "Workflow Router",
      "description": "Route requests into the phased workflow. Unified entry point for all workflow operations.",
      "arguments": [
        {
          "name": "task",
          "description": "The task or request to route through the workflow",
          "required": false
        }
      ]
    },
    {
      "name": "sy-constraints",
      "title": "Workflow Constraints",
      "description": "Apply baseline workflow constraints. Core safety and execution guards.",
      "arguments": []
    }
  ]
}
```

### 4.2 prompts/get 请求

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "prompts/get",
  "params": {
    "name": "sy-workflow",
    "arguments": { "task": "implement user authentication" }
  }
}
```

### 4.3 prompts/get 响应

来源：`specification/2025-11-25/server/prompts.mdx` §Protocol Messages

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "description": "Route requests into the phased workflow",
    "messages": [
      {
        "role": "user",
        "content": {
          "type": "text",
          "text": "<SKILL.md 内容，$ARGUMENTS 已替换为 'implement user authentication'>"
        }
      }
    ]
  }
}
```

`PromptMessage` 支持内容类型：`text`、`image`（base64）、`audio`（base64）、`resource`（嵌入资源）。

---

## 4.4 Skill 加载优先级（真实代码验证）

来源：`refer/agent-source-code/gemini-cli-main/packages/core/src/skills/skillManager.ts`

Gemini CLI `SkillManager.discoverSkills()` 六层加载顺序（优先级从低到高）：

```
1. builtin skills       ← 内置 skill（isBuiltin=true）
2. extension skills     ← 插件提供的 skill
3. user skills          ← ~/.gemini/skills/
4. user agent skills    ← ~/.agents/skills/
5. project skills       ← {workspace}/.gemini/skills/
6. project agent skills ← {workspace}/.agents/skills/  ← 最高优先级
```

**同名覆盖规则**：`addSkillsWithPrecedence()` 使用 Map 去重，后加载的同名 skill 覆盖先加载的（高优先级覆盖低优先级）。覆盖内置 skill 时发出 warn，覆盖项目 skill 时发出 conflict warning。

**安全约束**：`isTrusted=false`（workspace 不受信任）时，禁止加载 project skills 和 project agent skills，防止恶意 workspace 注入 skill。

**seeyue-mcp 对应关系**：`prompts/list` 返回的 skill 列表应遵循同样的优先级和覆盖语义。当 workspace 存在同名 skill 时，workspace 版本覆盖全局版本。`workflow/skills.spec.yaml` 作为 project-level source of truth，优先级最高。

---

## 5. Rust 实现

```rust
// prompts/skills.rs
async fn get_prompt(
    name: &str,
    arguments: Option<HashMap<String, String>>,
    skills_root: &Path,
) -> Result<GetPromptResult, McpError> {
    // 1. 查找 SKILL.md 路径
    let skill_path = skills_root.join(name).join("SKILL.md");
    if !skill_path.exists() {
        return Err(McpError::invalid_params(format!("Skill not found: {}", name), None));
    }

    // 2. 读取 SKILL.md
    let content = tokio::fs::read_to_string(&skill_path).await
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    // 3. 替换 $ARGUMENTS
    let resolved = resolve_arguments(content, arguments.unwrap_or_default());

    Ok(GetPromptResult {
        description: Some(format!("seeyue skill: {}", name)),
        messages: vec![
            PromptMessage {
                role: PromptMessageRole::User,
                content: PromptMessageContent::text(resolved),
            }
        ],
    })
}

fn resolve_arguments(content: String, args: HashMap<String, String>) -> String {
    // 替换 $ARGUMENTS、$0、$1 等位置参数
    // 来源：CLAUDE.md Skill Frontmatter 规范
    let mut result = content;
    if let Some(full_args) = args.get("ARGUMENTS").or(args.get("task")) {
        result = result.replace("$ARGUMENTS", full_args);
    }
    for (i, (_, v)) in args.iter().enumerate() {
        result = result.replace(&format!("${}", i), v);
    }
    result
}
```

**错误处理**（来源：`specification/2025-11-25/server/prompts.mdx` §Error Handling）：
- 无效 prompt name：`-32602`（Invalid params）
- 缺少必需参数：`-32602`（Invalid params）
- 内部错误：`-32603`（Internal error）

---

## 6. Skill Prompt 工程约束

来源：`refer/agent-source-code/claude-code-security-review-main/claudecode/prompts.py`

### 6.1 置信度阈值（>80% 原则）

Anthropic 官方 security review action 的 prompt 工程实践验证：**置信度阈值应在 prompt 层声明，而不是靠后处理过滤**。

```
✅ 正确：SKILL.md 中明确写 "只有当你 >80% 确信时才标记问题"
❌ 错误：让 Agent 随意输出再靠 hook 过滤低置信度内容
```

seeyue-workflows skill 中凡涉及判断/评审/验证场景的，SKILL.md 应包含明确的置信度约束：

```yaml
# SKILL.md frontmatter 约束示例
constraints:
  - "只在 >80% 确信时才报告问题，避免产生噪音"
  - "优先给出可操作的具体建议，不给模糊警告"
```

### 6.2 大 Context 降级策略

来源：`claudecode/prompts.py` diff 大小判断逻辑

当 skill 关联的文件内容超出 token 预算时，不应截断内容，而应切换到「工具探索」模式：

```
当 token_estimate > budget 时：
  ✅ 切换：不传完整内容，改为提示 Agent 使用 file_outline / read_range 工具探索
  ❌ 截断：直接截断内容传入，导致 Agent 基于不完整信息做判断
```

对应 `prompts/get` 实现：当 SKILL.md 包含 `max_context_tokens` 字段且当前 token 估算超限时，response 中附加降级提示：

```json
{
  "messages": [{
    "role": "user",
    "content": { "type": "text",
      "text": "[SKILL 内容] \n\nNOTE: 关联文件内容已省略（超出 context 预算）。请使用 file_outline / read_range / search_workspace 工具主动探索相关文件。"
    }
  }]
}
```

### 6.3 可注入的自定义约束插槽

来源：`claudecode/prompts.py` `custom_scan_instructions` 参数

SKILL.md 模板应预留 `$CUSTOM_INSTRUCTIONS` 插槽，支持外部注入额外约束而不修改 skill 主体：

```markdown
<!-- SKILL.md 末尾 -->
$CUSTOM_INSTRUCTIONS
```

`prompts/get` 调用时，`arguments` 中的 `custom_instructions` 字段会替换该占位符，实现 skill 主体与扩展约束的解耦。

## 6. Skills 目录约定

来源：`docs/skills-implementation-guide.md`，`CLAUDE.md` Skill Frontmatter 节

```
.agents/skills/
├── sy-workflow/
│   └── SKILL.md          # Prompt 内容来源
├── sy-constraints/
│   ├── SKILL.md
│   ├── appsec/SKILL.md   # 子 skill
│   ├── debug/SKILL.md
│   └── ...（12 个子约束）
├── sy-debug/
│   └── SKILL.md
└── ...
```

**disable-model-invocation**（来源：`CLAUDE.md` Skill Frontmatter）：

```yaml
# SKILL.md frontmatter
disable-model-invocation: true  # 标记为仅手动调用，不通过 MCP Prompts 暴露
```

带此标记的 skill 在 `prompts/list` 中不出现。

---

## 7. 渐进加载策略

来源：`workflow/skills.spec.yaml` skill_tiers

```rust
// prompts/skills.rs — 加载策略
pub fn list_prompts(skills_spec: &SkillsSpec, tier_filter: Option<&str>) -> Vec<Prompt> {
    skills_spec.skills.iter()
        .filter(|(_, skill)| {
            // 过滤 disable_model_invocation
            if skill.policy.disable_model_invocation { return false; }
            // 按 tier 过滤（None = 全部）
            match tier_filter {
                Some(t) => skill.tier.as_deref() == Some(t),
                None    => true,
            }
        })
        .map(|(id, skill)| Prompt {
            name: id.clone(),
            title: Some(skill.title.clone()),
            description: Some(skill.summary.clone()),
            arguments: build_arguments(skill),
            icons: None,  // 可选：从 skill 目录读取 icon 文件
        })
        .collect()
}
```
