# seeyue-workflows Skills 最佳实践

## 【文档定位】

本文档提供 Skills 开发和使用的最佳实践，帮助团队构建高质量的 Skills。

**相关文档**：
- `skills-architecture.md` - 架构设计
- `skills-catalog.md` - Skills 目录
- `skills-implementation-guide.md` - 实施指南

---

## 【第一章：设计原则】

### 1.1 单一职责原则

**❌ 错误示例：单体 Skill**
```yaml
name: sy-do-everything
description: Analyze, design, implement, test, and deploy
allowed-tools: [Read, Write, Edit, Bash, WebSearch]
```

**✅ 正确示例：职责分离**
```yaml
# 分析 Skill
name: sy-analyze-requirements
description: Analyze user requirements and generate user stories
allowed-tools: [Read, Write]

# 设计 Skill
name: sy-design-architecture
description: Design technical architecture based on requirements
allowed-tools: [Read, Write]
dependencies: [sy-analyze-requirements]

# 实现 Skill
name: sy-implement-feature
description: Implement feature based on approved design
allowed-tools: [Read, Write, Edit, Bash]
dependencies: [sy-design-architecture]
```

**原则**：
- 每个 Skill 只做一件事
- Skill 之间通过依赖关系组合
- 便于测试和维护

### 1.2 状态优先原则

**❌ 错误示例：依赖聊天历史**
```javascript
// 错误：假设之前讨论过技术栈
const framework = "React"; // 从哪里来的？
```

**✅ 正确示例：读取状态**
```javascript
// 正确：从状态中读取
const state = await readWorkflowState();
const framework = state.decisions.frontend_framework;

if (!framework) {
  throw new Error('Frontend framework not decided. Please run sy-design first.');
}
```

**原则**：
- 所有决策存储在状态中
- Skill 从状态读取输入
- Skill 更新状态记录输出

### 1.3 证据优先原则

**❌ 错误示例：声明完成**
```javascript
console.log("All tests passed");
```

**✅ 正确示例：提供证据**
```javascript
const testResult = await runTests();

// 记录证据到 journal
await appendJournal({
  event: 'test_execution',
  result: testResult.success ? 'pass' : 'fail',
  evidence: {
    total_tests: testResult.total,
    passed: testResult.passed,
    failed: testResult.failed,
    coverage: testResult.coverage,
    output_file: testResult.outputPath
  }
});

console.log(`Tests completed. Evidence: ${testResult.outputPath}`);
```

**原则**：
- 每个操作都有证据
- 证据存储在 journal.jsonl
- 证据可追溯和验证

### 1.4 可组合性原则

**❌ 错误示例：硬编码流程**
```javascript
async function deployApplication() {
  await buildCode();
  await runTests();
  await createDockerImage();
  await pushToRegistry();
  await deployToProduction();
}
```

**✅ 正确示例：可组合 Skills**
```yaml
# 定义独立 Skills
skills:
  - sy-build
  - sy-test
  - sy-containerize
  - sy-deploy

# 通过工作流组合
workflow:
  deployment:
    steps:
      - skill: sy-build
      - skill: sy-test
        depends_on: [sy-build]
      - skill: sy-containerize
        depends_on: [sy-test]
      - skill: sy-deploy
        depends_on: [sy-containerize]
```

**原则**：
- Skills 独立可用
- 通过依赖关系组合
- 支持不同的组合方式

---

## 【第二章：命名规范】

### 2.1 Skill 命名

**格式**：`sy-<category>-<action>`

**分类前缀**：
- `sy-workflow-*` - 工作流控制
- `sy-plan-*` - 计划相关
- `sy-exec-*` - 执行相关
- `sy-review-*` - 审查相关
- `sy-test-*` - 测试相关
- `sy-deploy-*` - 部署相关

**示例**：
```
✅ sy-plan-requirements    # 计划阶段的需求分析
✅ sy-exec-feature         # 执行阶段的功能实现
✅ sy-test-integration     # 集成测试
✅ sy-deploy-production    # 生产部署

❌ analyze-requirements    # 缺少 sy- 前缀
❌ sy-do-stuff            # 不明确的动作
❌ sy-feature             # 缺少分类
```

### 2.2 参数命名

**使用 snake_case**：
```yaml
✅ task_id
✅ target_environment
✅ dry_run

❌ taskId          # camelCase
❌ TargetEnv       # PascalCase
❌ target-env      # kebab-case
```

**参数类型标注**：
```markdown
## 输入参数

- `task_id` (string, 必需): 任务唯一标识符
- `options` (object, 可选): 执行选项
  - `dry_run` (boolean): 仅模拟执行，不实际修改
  - `verbose` (boolean): 输出详细日志
```

### 2.3 文件命名

**Skill 目录结构**：
```
.agents/skills/sy-example-skill/
├── SKILL.md           # 元数据和文档（必需）
├── execute.cjs        # 执行脚本（可选）
├── validate.cjs       # 验证脚本（可选）
├── templates/         # 模板文件（可选）
│   └── output.md
└── README.md          # 详细说明（可选）
```

---

## 【第三章：文档标准】

### 3.1 SKILL.md 模板

**完整模板**：
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
argument-hint: "[task_id, options]"
disable-model-invocation: false
requires-approval: false
dependencies:
  - sy-constraints
  - sy-workflow
---

# sy-example-skill

## 一句话描述

[简洁描述 Skill 的核心功能]

## 触发条件

当满足以下条件时调用此 Skill：
- 条件 1：[具体条件]
- 条件 2：[具体条件]
- 条件 3：[具体条件]

## 前置条件

- [ ] 工作流阶段为 `execute`
- [ ] 已执行 `sy-constraints`
- [ ] 存在待执行的任务

## 输入参数

### 必需参数

- `task_id` (string): 任务唯一标识符
  - 格式：`task-XXX`
  - 示例：`task-001`

### 可选参数

- `options` (object): 执行选项
  - `dry_run` (boolean): 仅模拟执行，默认 `false`
  - `verbose` (boolean): 详细输出，默认 `false`

## 执行流程

1. **验证前置条件**
   - 检查工作流阶段
   - 验证任务存在
   - 确认依赖 Skills 已执行

2. **读取任务详情**
   - 从执行计划读取任务
   - 解析任务依赖
   - 确定执行顺序

3. **执行任务步骤**
   - 步骤 1：[具体步骤]
   - 步骤 2：[具体步骤]
   - 步骤 3：[具体步骤]

4. **验证执行结果**
   - 运行测试
   - 检查输出
   - 验证状态

5. **记录证据**
   - 写入 journal.jsonl
   - 更新任务状态
   - 生成执行报告

## 输出

### 成功输出

\`\`\`json
{
  "success": true,
  "task_id": "task-001",
  "duration_ms": 1234,
  "evidence": {
    "files_modified": ["src/feature.js"],
    "tests_passed": 15,
    "journal_entry": "exec_20260312_103045"
  }
}
\`\`\`

### 失败输出

\`\`\`json
{
  "success": false,
  "task_id": "task-001",
  "error": "TDD Red Gate: No failing test found",
  "suggestion": "Please write a failing test first"
}
\`\`\`

## 示例

### 基本用法

\`\`\`bash
/sy-example-skill task-001
\`\`\`

### 使用选项

\`\`\`bash
/sy-example-skill task-001 --dry-run --verbose
\`\`\`

## 错误处理

| 错误代码 | 错误信息 | 解决方案 |
|---------|---------|---------|
| E001 | Invalid phase | 转换到 execute 阶段 |
| E002 | Task not found | 检查任务 ID |
| E003 | TDD Red Gate | 编写失败测试 |

## 相关 Skills

- `sy-constraints` - 必须先执行
- `sy-writing-plans` - 生成执行计划
- `sy-verification-before-completion` - 验证完成

## 更新日志

- 2026-03-12: 初始版本
- 2026-03-15: 添加 dry_run 选项
```

### 3.2 代码注释标准

**JavaScript/Node.js**：
```javascript
/**
 * 执行任务的核心逻辑
 *
 * @param {string} taskId - 任务 ID
 * @param {Object} options - 执行选项
 * @param {boolean} options.dryRun - 仅模拟执行
 * @param {boolean} options.verbose - 详细输出
 * @returns {Promise<Object>} 执行结果
 * @throws {Error} 当任务不存在或前置条件不满足时
 */
async function executeTask(taskId, options = {}) {
  // 1. 验证前置条件
  validatePreconditions();

  // 2. 读取任务详情
  const task = await loadTask(taskId);

  // 3. 执行任务
  const result = await performTask(task, options);

  // 4. 记录证据
  await recordEvidence(result);

  return result;
}
```

**PowerShell**：
```powershell
<#
.SYNOPSIS
    执行任务的核心逻辑

.DESCRIPTION
    根据任务 ID 执行相应的任务步骤，并记录执行证据

.PARAMETER TaskId
    任务唯一标识符

.PARAMETER DryRun
    仅模拟执行，不实际修改文件

.EXAMPLE
    Execute-Task -TaskId "task-001"

.EXAMPLE
    Execute-Task -TaskId "task-001" -DryRun
#>
function Execute-Task {
    param(
        [Parameter(Mandatory=$true)]
        [string]$TaskId,

        [Parameter(Mandatory=$false)]
        [switch]$DryRun
    )

    # 实现逻辑
}
```

---

## 【第四章：错误处理】

### 4.1 错误分类

**三类错误**：

1. **用户错误**（User Error）：
   - 参数错误
   - 前置条件不满足
   - 权限不足

2. **系统错误**（System Error）：
   - 文件不存在
   - 网络故障
   - 依赖服务不可用

3. **逻辑错误**（Logic Error）：
   - 测试失败
   - 验证不通过
   - 状态不一致

### 4.2 错误处理模式

**模式 1：快速失败**
```javascript
async function executeSkill(args) {
  // 验证参数
  if (!args.taskId) {
    throw new Error('Missing required parameter: taskId');
  }

  // 验证前置条件
  const state = await readWorkflowState();
  if (state.phase.current !== 'execute') {
    throw new Error('Invalid phase: must be in execute phase');
  }

  // 执行逻辑
  // ...
}
```

**模式 2：优雅降级**
```javascript
async function executeSkill(args) {
  try {
    // 尝试主要逻辑
    return await primaryLogic(args);
  } catch (err) {
    console.warn('Primary logic failed, trying fallback:', err.message);

    // 回退逻辑
    return await fallbackLogic(args);
  }
}
```

**模式 3：部分成功**
```javascript
async function executeBatchTasks(taskIds) {
  const results = {
    succeeded: [],
    failed: []
  };

  for (const taskId of taskIds) {
    try {
      await executeTask(taskId);
      results.succeeded.push(taskId);
    } catch (err) {
      results.failed.push({ taskId, error: err.message });
    }
  }

  return results;
}
```

### 4.3 错误消息规范

**格式**：`[错误类型] 错误描述 - 解决建议`

**示例**：
```javascript
// ❌ 不好的错误消息
throw new Error('Invalid input');

// ✅ 好的错误消息
throw new Error(
  '[User Error] Invalid task_id format: expected "task-XXX", got "invalid" - ' +
  'Please provide a valid task ID in the format task-001, task-002, etc.'
);
```

**结构化错误**：
```javascript
class SkillError extends Error {
  constructor(type, message, suggestion) {
    super(message);
    this.type = type;  // 'user' | 'system' | 'logic'
    this.suggestion = suggestion;
  }

  toJSON() {
    return {
      success: false,
      error: {
        type: this.type,
        message: this.message,
        suggestion: this.suggestion
      }
    };
  }
}

// 使用
throw new SkillError(
  'user',
  'Task not found: task-999',
  'Please check the task ID in .ai/plans/execution-plan.yaml'
);
```

---

## 【第五章：性能优化】

### 5.1 缓存策略

**缓存工作流状态**：
```javascript
// ❌ 每次都读取文件
async function getPhase() {
  const state = await readWorkflowState();
  return state.phase.current;
}

// ✅ 缓存状态（1 秒过期）
const stateCache = {
  data: null,
  timestamp: 0,
  ttl: 1000  // 1 秒
};

async function getPhase() {
  const now = Date.now();
  if (stateCache.data && (now - stateCache.timestamp) < stateCache.ttl) {
    return stateCache.data.phase.current;
  }

  stateCache.data = await readWorkflowState();
  stateCache.timestamp = now;
  return stateCache.data.phase.current;
}
```

### 5.2 并行执行

**并行读取多个文件**：
```javascript
// ❌ 串行读取
const file1 = await readFile('file1.txt');
const file2 = await readFile('file2.txt');
const file3 = await readFile('file3.txt');

// ✅ 并行读取
const [file1, file2, file3] = await Promise.all([
  readFile('file1.txt'),
  readFile('file2.txt'),
  readFile('file3.txt')
]);
```

### 5.3 增量处理

**仅处理变更的文件**：
```javascript
async function processFiles() {
  // 读取上次处理的时间戳
  const lastProcessed = await getLastProcessedTimestamp();

  // 仅处理变更的文件
  const changedFiles = await getChangedFilesSince(lastProcessed);

  for (const file of changedFiles) {
    await processFile(file);
  }

  // 更新时间戳
  await setLastProcessedTimestamp(Date.now());
}
```

---

## 【第六章：安全实践】

### 6.1 输入验证

**验证所有输入**：
```javascript
function validateTaskId(taskId) {
  // 格式验证
  if (!/^task-\d{3}$/.test(taskId)) {
    throw new Error(`Invalid task ID format: ${taskId}`);
  }

  // 范围验证
  const taskNumber = parseInt(taskId.split('-')[1]);
  if (taskNumber < 1 || taskNumber > 999) {
    throw new Error(`Task ID out of range: ${taskId}`);
  }

  return taskId;
}
```

### 6.2 路径安全

**防止路径遍历攻击**：
```javascript
const path = require('path');

function safeReadFile(filePath) {
  // 规范化路径
  const normalized = path.normalize(filePath);

  // 确保在项目目录内
  const projectRoot = process.cwd();
  const absolute = path.resolve(projectRoot, normalized);

  if (!absolute.startsWith(projectRoot)) {
    throw new Error(`Path traversal detected: ${filePath}`);
  }

  return fs.readFileSync(absolute, 'utf8');
}
```

### 6.3 命令注入防护

**使用参数化命令**：
```javascript
const { spawn } = require('child_process');

// ❌ 命令注入风险
const userInput = 'file.txt; rm -rf /';
exec(`cat ${userInput}`);  // 危险！

// ✅ 安全的参数化
spawn('cat', [userInput]);  // 安全
```

---

## 【第七章：测试策略】

### 7.1 测试金字塔

```
        ┌─────────┐
        │  E2E    │  10%
        │  Tests  │
        ├─────────┤
        │Integration│  30%
        │  Tests    │
        ├───────────┤
        │   Unit     │  60%
        │   Tests    │
        └───────────┘
```

**单元测试**：测试单个函数
**集成测试**：测试 Skill 与工作流集成
**E2E 测试**：测试完整开发流程

### 7.2 测试覆盖率目标

| 类型 | 目标覆盖率 |
|------|-----------|
| 核心逻辑 | 90%+ |
| 工具函数 | 80%+ |
| 集成代码 | 70%+ |
| UI/交互 | 50%+ |

### 7.3 测试命名规范

**格式**：`should_<expected_behavior>_when_<condition>`

**示例**：
```javascript
describe('sy-executing-plans', () => {
  it('should_execute_task_successfully_when_preconditions_met', async () => {
    // 测试逻辑
  });

  it('should_throw_error_when_task_not_found', async () => {
    // 测试逻辑
  });

  it('should_skip_task_when_already_completed', async () => {
    // 测试逻辑
  });
});
```

---

## 【第八章：Windows 平台优化】

### 8.1 路径处理

**使用 path 模块**：
```javascript
const path = require('path');

// ✅ 跨平台兼容
const filePath = path.join(process.cwd(), '.ai', 'workflow', 'session.yaml');

// ❌ 硬编码路径分隔符
const filePath = process.cwd() + '/.ai/workflow/session.yaml';
```

### 8.2 PowerShell 集成

**调用 PowerShell 脚本**：
```javascript
const { execSync } = require('child_process');

function runPowerShellScript(scriptPath, args = []) {
  const command = [
    'powershell.exe',
    '-ExecutionPolicy', 'Bypass',
    '-File', `"${scriptPath}"`,
    ...args.map(arg => `"${arg}"`)
  ].join(' ');

  return execSync(command, { encoding: 'utf8' });
}
```

### 8.3 注册表访问

**读写注册表**：
```javascript
function getRegistryValue(key, valueName) {
  const output = execSync(
    `reg query "${key}" /v "${valueName}"`,
    { encoding: 'utf8' }
  );

  const match = output.match(/REG_SZ\s+(.+)/);
  return match ? match[1].trim() : null;
}

function setRegistryValue(key, valueName, value) {
  execSync(
    `reg add "${key}" /v "${valueName}" /t REG_SZ /d "${value}" /f`,
    { stdio: 'ignore' }
  );
}
```

---

## 【第九章：持续改进】

### 9.1 收集指标

**Skill 执行指标**：
```javascript
const metrics = {
  skill_name: 'sy-example-skill',
  execution_count: 0,
  success_count: 0,
  failure_count: 0,
  total_duration_ms: 0,
  avg_duration_ms: 0
};

async function executeWithMetrics(skill, args) {
  const startTime = Date.now();
  metrics.execution_count++;

  try {
    const result = await skill.execute(args);
    metrics.success_count++;
    return result;
  } catch (err) {
    metrics.failure_count++;
    throw err;
  } finally {
    const duration = Date.now() - startTime;
    metrics.total_duration_ms += duration;
    metrics.avg_duration_ms = metrics.total_duration_ms / metrics.execution_count;

    // 保存指标到注册表
    saveMetrics(metrics);
  }
}
```

### 9.2 用户反馈

**收集用户反馈**：
```javascript
async function requestFeedback(skillName, result) {
  console.log(`\n${skillName} completed.`);
  console.log('Was this helpful? (yes/no/skip)`);

  // 在实际实现中，这里应该通过 UI 或 CLI 交互收集反馈
  const feedback = await getUserInput();

  if (feedback !== 'skip') {
    await saveFeedback({
      skill: skillName,
      helpful: feedback === 'yes',
      timestamp: new Date().toISOString()
    });
  }
}
```

### 9.3 定期审查

**审查清单**：
- [ ] Skill 使用频率分析
- [ ] 成功率和失败率统计
- [ ] 平均执行时间趋势
- [ ] 用户反馈汇总
- [ ] 错误日志分析
- [ ] 性能瓶颈识别
- [ ] 文档完整性检查
- [ ] 测试覆盖率评估

---

**文档版本**：v1.0.0
**最后更新**：2026-03-12
**作者**：seeyue-workflows 架构团队

---

**END OF DOCUMENT**
