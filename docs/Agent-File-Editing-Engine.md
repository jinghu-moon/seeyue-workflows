# 一、目标系统（Agent File Editing Engine）

目标：

```text
让 AI agent 调用工具修改文件
同时兼容 Claude Code 和 Codex
```

最终结构：

```text
           Agent
   (Claude / Codex / others)
                │
                │ tool call
                ▼
         Agent Tool Server
                │
        ┌───────┴────────┐
        │                │
   Claude Adapter    Codex Adapter
        │                │
        └───────┬────────┘
                ▼
          Editing Engine
                │
        ┌───────┴────────┐
        │                │
         Sandbox FS   History
                │
                ▼
            Filesystem
```

核心思想：

```
AI 只发“编辑请求”
系统统一转换为 EditOperation
Engine 执行
```

------

# 二、Claude Code 编辑机制

Claude Code 的官方工具叫：

```
text_editor
```

Claude 可以：

```
view
str_replace
insert
create
```

例如：

```json
{
 "command": "str_replace",
 "path": "src/main.rs",
 "old_str": "println!(\"Hello\");",
 "new_str": "println!(\"Hello world\");"
}
```

Claude workflow：

```
view file
analyze
edit file
```

Claude **不会直接写文件**，而是调用工具执行编辑。 ([Claude API Docs](https://docs.anthropic.com/en/docs/agents-and-tools/tool-use/text-editor-tool?utm_source=chatgpt.com))

------

# 三、Codex 编辑机制

Codex CLI 使用：

```
apply_patch
```

例如：

```
*** Begin Patch
*** Update File: src/main.rs
@@
-println!("Hello");
+println!("Hello world");
*** End Patch
```

所以系统要支持 **两种输入协议**：

| Agent  | 协议        |
| ------ | ----------- |
| Claude | text_editor |
| Codex  | apply_patch |

------

# 四、统一内部数据模型

所有编辑统一转换为：

```rust
enum EditOperation {

    Replace {
        path: String,
        old: String,
        new: String
    },

    Insert {
        path: String,
        line: usize,
        content: String
    },

    Delete {
        path: String,
        start: usize,
        end: usize
    },

    Create {
        path: String,
        content: String
    }

}
```

这样：

```
所有 agent → EditOperation
```

------

# 五、Editing Engine

核心职责：

```
validate
load file
apply edit
write file
record history
```

Rust 示例：

```rust
fn apply_operation(op: EditOperation) -> Result<()> {

    match op {

        EditOperation::Replace { path, old, new } => {

            let text = std::fs::read_to_string(&path)?;

            let updated = text.replace(&old, &new);

            std::fs::write(&path, updated)?;

        }

        EditOperation::Insert { path, line, content } => {

            let mut lines: Vec<String> =
                std::fs::read_to_string(&path)?
                .lines()
                .map(|s| s.to_string())
                .collect();

            lines.insert(line, content);

            std::fs::write(&path, lines.join("\n"))?;
        }

        _ => {}

    }

    Ok(())
}
```

------

# 六、Claude Adapter

Claude tool call：

```json
{
 "command": "str_replace",
 "path": "src/main.rs",
 "old_str": "a",
 "new_str": "b"
}
```

转换：

```rust
fn claude_to_edit(req: ClaudeRequest) -> EditOperation {

    match req.command.as_str() {

        "str_replace" => EditOperation::Replace {
            path: req.path,
            old: req.old_str,
            new: req.new_str
        },

        "insert" => EditOperation::Insert {
            path: req.path,
            line: req.line,
            content: req.content
        },

        _ => panic!("unsupported")

    }
}
```

------

# 七、Codex Adapter

输入：

```
*** Update File: main.rs
@@
-old
+new
```

解析：

```rust
fn codex_patch_to_edit(patch: Patch) -> Vec<EditOperation> {

    let mut ops = Vec::new();

    for hunk in patch.hunks {

        ops.push(EditOperation::Replace {
            path: patch.path.clone(),
            old: hunk.old,
            new: hunk.new
        });

    }

    ops
}
```

------

# 八、Agent Tool API

你的工具可以提供：

```
read_file
list_files
apply_edit
apply_patch
```

示例：

### Claude 调用

```json
{
 "tool": "text_editor",
 "command": "str_replace",
 "path": "main.rs",
 "old_str": "==",
 "new_str": "==="
}
```

------

### Codex 调用

```
apply_patch
*** Begin Patch
*** Update File: main.rs
@@
-old
+new
*** End Patch
```

------

# 九、完整 Agent 调用流程

完整流程：

```
User prompt
     │
Agent reasoning
     │
view_file
     │
Agent analyze
     │
apply_edit
     │
Tool result
     │
Agent verify
```

Claude 的典型流程：

```
view
view_range
str_replace
```

------

# 十、安全设计（必须）

Agent editing engine 必须 sandbox。

### 1 root workspace

```
/workspace
```

------

### 2 path canonicalization

```rust
let canonical = path.canonicalize()?;
```

禁止：

```
../../etc/passwd
```

------

### 3 whitelist

允许：

```
src/
tests/
docs/
```

------

### 4 diff preview

执行前：

```
show diff
confirm
```

------

### 5 rollback

记录：

```
patch history
```

------

# 十一、推荐目录结构

```text
agent-editor
│
├─ src
│
├─ engine
│   ├─ edit.rs
│   ├─ apply.rs
│   ├─ history.rs
│
├─ adapters
│   ├─ claude.rs
│   ├─ codex.rs
│
├─ sandbox
│   ├─ fs.rs
│
└─ cli
    └─ main.rs
```

------

# 十二、未来升级（强烈推荐）

下一代 Agent editing engine 会加入：

### AST editing

不是文本：

```
rename_function
add_import
modify_struct
```

------

### fuzzy patch

上下文不完全匹配也能应用。

------

### semantic diff

AI 更容易生成。

------

# 十三、最终架构（最佳实践）

最终推荐架构：

```
          Agent
    (Claude / Codex)
           │
           ▼
       Tool Server
           │
    ┌──────┴───────┐
    │              │
 Claude Adapter  Codex Adapter
    │              │
    └──────┬───────┘
           ▼
       EditOperation
           │
           ▼
       Patch Engine
           │
           ▼
        Filesystem
```

核心原则：

```
AI 不直接写文件
AI 只发编辑请求
系统统一执行
```

------

✅ **一句话总结**

要适配 **Claude Code + Codex**：

```
统一 Editing Engine
+ Claude adapter
+ Codex adapter
```

这是目前 **Cursor / Copilot / Claude Code 的核心设计模式**。

