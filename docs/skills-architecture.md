# seeyue-workflows Skills 系统架构设计

## 【文档定位】

本文档定义 seeyue-workflows 的 Skills 系统架构，面向架构师和高级开发者。

**核心目标**：
1. 提供可组合、可扩展的 Skills 架构
2. 支持完整的项目开发流程（需求→设计→开发→测试→部署）
3. 与 V4 工作流状态模型深度集成
4. 100% 专注 Windows 平台优化

**相关文档**：
- `skills-catalog.md` - Skills 完整目录
- `skills-development-workflow.md` - 开发流程 Skills
- `skills-implementation-guide.md` - 实施指南
- `skills-best-practices.md` - 最佳实践

---

## 【第一章：架构总览】

### 1.1 六层分离架构

```
┌─────────────────────────────────────────────────────────┐
│  编排层 (Orchestration Layer)                           │
│  - sy-workflow: 统一入口和路由                          │
│  - 阶段转换控制                                         │
│  - 状态管理                                             │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│  计划层 (Planning Layer)                                │
│  - sy-ideation: 需求分析                                │
│  - sy-design: 技术设计                                  │
│  - sy-writing-plans: 执行计划                           │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│  执行层 (Execution Layer)                               │
│  - sy-executing-plans: 计划执行                         │
│  - sy-debug: 调试                                       │
│  - sy-verification-before-completion: 验证              │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│  协作层 (Collaboration Layer)                           │
│  - sy-requesting-code-review: 请求审查                  │
│  - sy-receiving-code-review: 处理反馈                   │
│  - sy-doc-sync: 文档同步                                │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│  约束层 (Constraints Layer)                             │
│  - sy-constraints: 12 个子约束                          │
│  - 硬守卫和软建议                                       │
└──────────────────┬──────────────────────────────────────┘
                   │
┌──────────────────▼──────────────────────────────────────┐
│  Hook 层 (Hooks Layer)                                  │
│  - PreToolUse: 工具调用前拦截                           │
│  - PostToolUse: 工具调用后验证                          │
│  - Stop: 阶段完成验证                                   │
└─────────────────────────────────────────────────────────┘
```

### 1.2 核心设计原则

**1. 状态优先于聊天历史**
```yaml
# 错误做法：依赖聊天历史
"根据之前的讨论，我们决定使用 React"

# 正确做法：读取状态
state = read_workflow_state()
framework = state.decisions.frontend_framework  # "React"
```

**2. 证据优先于声明**
```yaml
# 错误做法：声明完成
"我已经完成了所有测试"

# 正确做法：提供证据
evidence:
  - test_output: "All 15 tests passed"
  - coverage_report: "95% line coverage"
  - journal_entry: "test_run_20260312_103045"
```

**3. 约束优先于建议**
```yaml
# 软建议（可忽略）
"建议使用 TypeScript"

# 硬约束（必须遵守）
constraints:
  - name: "TDD Red Gate"
    type: "MUST"
    enforcement: "PreToolUse hook blocks Write without RED evidence"
```

**4. 可组合性优先于单体**
```yaml
# 错误做法：单体 Skill
sy-do-everything:
  - analyze requirements
  - design architecture
  - write code
  - run tests
  - deploy

# 正确做法：可组合 Skills
workflow:
  - sy-ideation
  - sy-design
  - sy-writing-plans
  - sy-executing-plans
  - sy-verification-before-completion
```

### 1.3 与 V4 工作流集成

**V4 状态模型**：
```yaml
session:
  phase:
    current: execute
    allowed_next: [review, done]

  tasks:
    - id: task-001
      status: in_progress
      owner: sy-executing-plans

  approvals:
    design: approved
    plan: approved

  evidence:
    red_ready: true
    test_coverage: 95
```

**Skill 与状态交互**：
```javascript
// Skill 读取状态
const state = await readWorkflowState();
if (state.phase.current !== 'execute') {
  throw new Error('Cannot execute plans in non-execute phase');
}

// Skill 更新状态
await updateWorkflowState({
  tasks: [
    ...state.tasks,
    { id: 'task-002', status: 'completed', owner: 'sy-executing-plans' }
  ]
});
```

---

## 【第二章：Skill 元数据规范】

### 2.1 SKILL.md 格式

**完整示例**：
```markdown
---
name: sy-example-skill
description: Use when [trigger condition] - [what it does]
category: execution
phase: execute
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
argument-hint: "[task_id, options]"
disable-model-invocation: false
requires-approval: false
dependencies:
  - sy-constraints
  - sy-workflow
---

# sy-example-skill

## 触发条件

当满足以下条件时调用此 Skill：
- 工作流阶段为 `execute`
- 存在待执行的任务
- 已通过 TDD Red Gate

## 输入参数

- `task_id` (必需): 任务 ID
- `options` (可选): 执行选项
  - `dry_run`: 仅模拟执行
  - `verbose`: 详细输出

## 执行流程

1. 读取任务详情
2. 验证前置条件
3. 执行任务步骤
4. 记录证据
5. 更新状态

## 输出

- 任务执行结果
- 证据链接
- 状态更新确认

## 示例

\`\`\`bash
/sy-example-skill task-001 --verbose
\`\`\`
```

### 2.2 元数据字段说明

| 字段 | 类型 | 必需 | 说明 |
|------|------|------|------|
| `name` | string | 是 | Skill 唯一标识符 |
| `description` | string | 是 | 触发条件和功能描述 |
| `category` | enum | 是 | orchestration/planning/execution/collaboration/constraints |
| `phase` | enum | 否 | 适用的工作流阶段 |
| `allowed-tools` | array | 是 | 允许使用的工具列表 |
| `argument-hint` | string | 否 | 参数提示 |
| `disable-model-invocation` | boolean | 否 | 是否禁用模型调用 |
| `requires-approval` | boolean | 否 | 是否需要人工批准 |
| `dependencies` | array | 否 | 依赖的其他 Skills |

### 2.3 分类体系

**五大类别**：

1. **Orchestration（编排）**：
   - 工作流控制
   - 阶段转换
   - 状态管理

2. **Planning（计划）**：
   - 需求分析
   - 技术设计
   - 任务分解

3. **Execution（执行）**：
   - 代码实现
   - 测试执行
   - 调试修复

4. **Collaboration（协作）**：
   - 代码审查
   - 文档同步
   - 团队沟通

5. **Constraints（约束）**：
   - 硬守卫
   - 软建议
   - 策略执行

---

## 【第三章：Skill 生命周期】

### 3.1 调用流程

```
用户输入 "/sy-example-skill task-001"
    ↓
sy-workflow 路由器
    ↓
验证阶段和权限
    ↓
加载 Skill 元数据
    ↓
检查依赖 Skills
    ↓
执行 PreToolUse hooks
    ↓
调用 Skill 逻辑
    ↓
执行 PostToolUse hooks
    ↓
更新工作流状态
    ↓
返回结果给用户
```

### 3.2 状态转换

**Skill 执行状态**：
```yaml
skill_execution:
  id: exec-001
  skill_name: sy-executing-plans
  status: in_progress  # pending/in_progress/completed/failed
  started_at: "2026-03-12T10:00:00Z"
  completed_at: null
  evidence:
    - type: test_output
      path: ".ai/evidence/test-001.log"
```

**状态机**：
```
pending → in_progress → completed
                ↓
              failed → rollback
```

### 3.3 错误处理

**三层错误处理**：

1. **Skill 内部错误**：
```javascript
try {
  await executeTask(taskId);
} catch (error) {
  await logError(error);
  await rollbackChanges();
  throw new SkillExecutionError(error.message);
}
```

2. **Hook 拦截错误**：
```javascript
// PreToolUse hook 阻断
{
  "decision": "deny",
  "reason": "TDD Red Gate: No failing test found",
  "systemMessage": "Please write a failing test first"
}
```

3. **工作流级错误**：
```javascript
// 阶段转换失败
{
  "error": "Cannot transition from 'plan' to 'done'",
  "allowed_transitions": ["execute", "review"],
  "recovery": "Complete execution phase first"
}
```

---

## 【第四章：Skill 组合模式】

### 4.1 串行组合

**示例：完整开发流程**
```yaml
workflow:
  name: "Feature Development"
  mode: sequential
  skills:
    - sy-ideation:
        input: "Add user authentication"
        output: requirements_doc

    - sy-design:
        input: requirements_doc
        output: technical_design

    - sy-writing-plans:
        input: technical_design
        output: execution_plan

    - sy-executing-plans:
        input: execution_plan
        output: implementation

    - sy-verification-before-completion:
        input: implementation
        output: verification_report
```

### 4.2 并行组合

**示例：多任务并行执行**
```yaml
workflow:
  name: "Parallel Tasks"
  mode: parallel
  skills:
    - sy-executing-plans:
        task_id: task-001  # Frontend

    - sy-executing-plans:
        task_id: task-002  # Backend

    - sy-executing-plans:
        task_id: task-003  # Database
```

### 4.3 条件组合

**示例：基于状态的条件执行**
```yaml
workflow:
  name: "Conditional Execution"
  skills:
    - sy-executing-plans:
        task_id: task-001

    - if: state.evidence.test_failed
      then:
        - sy-debug:
            error_log: ".ai/evidence/test-001.log"
      else:
        - sy-verification-before-completion
```

### 4.4 循环组合

**示例：TDD 循环**
```yaml
workflow:
  name: "TDD Cycle"
  loop:
    condition: "!state.evidence.all_tests_passed"
    max_iterations: 10
    skills:
      - sy-executing-plans:
          phase: red  # Write failing test

      - sy-executing-plans:
          phase: green  # Make test pass

      - sy-executing-plans:
          phase: refactor  # Improve code
```

---

## 【第五章：Windows 平台优化】

### 5.1 路径处理

**Windows 路径规范化**：
```javascript
// .agents/skills/lib/path-utils.cjs
const path = require('path');

function normalizeWindowsPath(inputPath) {
  // 保持 Windows 原生格式
  return path.normalize(inputPath);
}

function resolveSkillPath(relativePath) {
  const skillsRoot = path.join(process.cwd(), '.agents/skills');
  return path.resolve(skillsRoot, relativePath);
}

module.exports = { normalizeWindowsPath, resolveSkillPath };
```

### 5.2 PowerShell 集成

**Skill 调用 PowerShell 脚本**：
```javascript
// .agents/skills/sy-example-skill/execute.cjs
const { execSync } = require('child_process');

function runPowerShellScript(scriptPath, args) {
  const command = `powershell.exe -ExecutionPolicy Bypass -File "${scriptPath}" ${args.join(' ')}`;

  try {
    const output = execSync(command, { encoding: 'utf8' });
    return { success: true, output };
  } catch (error) {
    return { success: false, error: error.message };
  }
}
```

### 5.3 注册表集成

**Skill 状态存储到注册表**：
```javascript
// .agents/skills/lib/registry-store.cjs
const { RegistryStore } = require('../../../scripts/runtime/registry-store.cjs');

class SkillStateStore {
  constructor(skillName) {
    this.store = new RegistryStore(`HKCU\\Software\\seeyue\\skills\\${skillName}`);
  }

  saveState(state) {
    this.store.set('state', state);
  }

  loadState() {
    return this.store.get('state');
  }
}

module.exports = { SkillStateStore };
```

### 5.4 Windows 事件日志

**Skill 执行记录到事件日志**：
```javascript
// .agents/skills/lib/event-logger.cjs
const { execSync } = require('child_process');

function logSkillExecution(skillName, status, details) {
  const eventType = status === 'success' ? 'Information' : 'Warning';
  const eventId = status === 'success' ? 1100 : 2100;

  const psScript = `
    Write-EventLog -LogName Application -Source "seeyue-workflows" \`
      -EntryType ${eventType} -EventId ${eventId} \`
      -Message "Skill: ${skillName}\nStatus: ${status}\nDetails: ${JSON.stringify(details)}"
  `;

  try {
    execSync(`powershell.exe -Command "${psScript}"`, { stdio: 'ignore' });
  } catch (error) {
    // 静默失败
  }
}

module.exports = { logSkillExecution };
```

---

## 【第六章：与 MCP 集成】

### 6.1 Skills 作为 MCP Prompts

**设计理念**：将 Skills 暴露为 MCP Prompts，供 AI 客户端调用

**映射关系**：
```
seeyue Skill → MCP Prompt
sy-ideation → prompt://seeyue/ideation
sy-design → prompt://seeyue/design
sy-executing-plans → prompt://seeyue/execute
```

**实现示例（Rust）**：
```rust
// seeyue-mcp/src/prompts/skills.rs
#[prompt_handler]
pub async fn ideation_prompt(
    &self,
    #[arg(description = "Feature description")] feature: String,
) -> Result<PromptContent, EngineError> {
    // 读取 Skill 模板
    let template = self.load_skill_template("sy-ideation").await?;

    // 渲染模板
    let rendered = template.replace("{{feature}}", &feature);

    Ok(PromptContent {
        name: "ideation".to_string(),
        description: "Analyze requirements and create specification".to_string(),
        messages: vec![
            PromptMessage {
                role: "user".to_string(),
                content: rendered,
            }
        ],
    })
}
```

### 6.2 Skills 作为 MCP Tools

**设计理念**：将 Skills 暴露为 MCP Tools，供 AI 客户端执行

**映射关系**：
```
seeyue Skill → MCP Tool
sy-executing-plans → tool://seeyue/execute_plan
sy-debug → tool://seeyue/debug
sy-verification-before-completion → tool://seeyue/verify
```

**实现示例（Rust）**：
```rust
// seeyue-mcp/src/tools/skills.rs
#[tool_handler]
pub async fn execute_plan(
    &self,
    #[arg(description = "Task ID")] task_id: String,
) -> Result<ToolResponse, EngineError> {
    // 调用 Node.js Skill
    let output = Command::new("node")
        .arg(".agents/skills/sy-executing-plans/execute.cjs")
        .arg(&task_id)
        .current_dir(&self.workspace)
        .output()
        .await?;

    let result: SkillResult = serde_json::from_slice(&output.stdout)?;

    Ok(ToolResponse::text(format!(
        "Task {} executed: {}",
        task_id, result.status
    )))
}
```

---

## 【第七章：总结与建议】

### 7.1 核心价值

本架构设计提供以下核心价值：

1. **六层分离架构**：清晰的职责划分，易于扩展
2. **状态驱动**：基于 V4 工作流状态，而非聊天历史
3. **可组合性**：Skills 可串行、并行、条件、循环组合
4. **Windows 优化**：充分利用注册表、PowerShell、事件日志
5. **MCP 集成**：通过 MCP 协议暴露 Skills 能力

### 7.2 实施建议

**推荐路径**：
1. **Phase 1（P0）**：完善现有 17 个 Skills 的元数据
2. **Phase 2（P1）**：实现 8 个新增 Skills（见 skills-catalog.md）
3. **Phase 3（P1）**：实现 Skill 组合引擎
4. **Phase 4（P2）**：MCP 集成
5. **Phase 5（P2）**：Windows 优化和性能调优

**关键成功因素**：
1. **元数据标准化**：所有 Skills 遵循统一的 SKILL.md 格式
2. **状态优先**：Skills 读写 V4 工作流状态，而非依赖聊天历史
3. **证据驱动**：所有声明都有证据支持
4. **约束执行**：通过 Hooks 强制执行硬约束

### 7.3 下一步行动

**本周**：
- [ ] 评审本架构设计
- [ ] 完善现有 Skills 的 SKILL.md 元数据
- [ ] 确定新增 Skills 的优先级

**下月**：
- [ ] 实现 Skill 组合引擎
- [ ] 开发 3-5 个新增 Skills
- [ ] 编写 Skills 单元测试

**三个月**：
- [ ] 完成所有新增 Skills
- [ ] MCP 集成
- [ ] 性能优化和文档完善

---

**文档版本**：v1.0.0
**最后更新**：2026-03-12
**作者**：seeyue-workflows 架构团队

---

**END OF DOCUMENT**
