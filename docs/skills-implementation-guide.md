# seeyue-workflows Skills 实施指南

## 【文档定位】

本文档提供 Skills 系统的实施指南，包括开发、测试、部署的详细步骤。

**相关文档**：
- `skills-architecture.md` - 架构设计
- `skills-catalog.md` - Skills 目录
- `skills-development-workflow.md` - 开发流程

---

## 【第一章：开发新 Skill】

### 1.1 创建 Skill 目录结构

**步骤 1：创建目录**
```powershell
# 在 .agents/skills/ 下创建新 Skill
$skillName = "sy-example-skill"
New-Item -ItemType Directory -Path ".agents/skills/$skillName"
```

**步骤 2：创建 SKILL.md**
```powershell
# 创建元数据文件
@"
---
name: $skillName
description: Use when [trigger condition] - [what it does]
category: execution
phase: execute
allowed-tools:
  - Read
  - Write
  - Edit
  - Bash
argument-hint: "[arg1, arg2]"
disable-model-invocation: false
requires-approval: false
dependencies: []
---

# $skillName

## 触发条件

当满足以下条件时调用此 Skill：
- 条件 1
- 条件 2

## 输入参数

- \`arg1\` (必需): 参数 1 说明
- \`arg2\` (可选): 参数 2 说明

## 执行流程

1. 步骤 1
2. 步骤 2
3. 步骤 3

## 输出

- 输出 1
- 输出 2

## 示例

\`\`\`bash
/$skillName arg1 arg2
\`\`\`
"@ | Set-Content ".agents/skills/$skillName/SKILL.md"
```

### 1.2 实现 Skill 逻辑

**选项 A：纯提示型 Skill（无代码）**

仅需 SKILL.md，Claude 根据提示执行。

**示例**：
```markdown
---
name: sy-simple-analysis
description: Use when need to analyze code complexity
category: planning
phase: plan
allowed-tools: [Read, Grep]
---

# sy-simple-analysis

## 执行流程

1. 使用 Grep 搜索所有函数定义
2. 统计每个函数的行数
3. 识别超过 50 行的函数
4. 生成复杂度报告

## 输出格式

\`\`\`markdown
# 代码复杂度分析

## 高复杂度函数
- \`functionName\` (120 行) - src/module.js:45
- \`anotherFunction\` (85 行) - src/utils.js:12

## 建议
- 重构 \`functionName\`，拆分为多个小函数
\`\`\`
```

**选项 B：脚本型 Skill（有代码）**

创建辅助脚本处理复杂逻辑。

**步骤 1：创建脚本**
```powershell
# 创建 Node.js 脚本
@"
#!/usr/bin/env node
// .agents/skills/sy-example-skill/execute.cjs

const fs = require('fs');
const path = require('path');

async function main() {
  // 解析参数
  const args = process.argv.slice(2);
  const arg1 = args[0];
  const arg2 = args[1];

  console.log(\`Executing with arg1=\${arg1}, arg2=\${arg2}\`);

  // 读取工作流状态
  const statePath = path.join(process.cwd(), '.ai/workflow/session.yaml');
  const state = fs.readFileSync(statePath, 'utf8');

  // 执行 Skill 逻辑
  const result = await executeSkillLogic(arg1, arg2, state);

  // 输出结果
  console.log(JSON.stringify(result, null, 2));
}

async function executeSkillLogic(arg1, arg2, state) {
  // 实现具体逻辑
  return {
    success: true,
    output: 'Skill executed successfully'
  };
}

main().catch(err => {
  console.error('Error:', err.message);
  process.exit(1);
});
"@ | Set-Content ".agents/skills/$skillName/execute.cjs"
```

**步骤 2：在 SKILL.md 中引用脚本**
```markdown
## 执行方式

此 Skill 使用辅助脚本执行：

\`\`\`bash
node .agents/skills/sy-example-skill/execute.cjs arg1 arg2
\`\`\`
```

### 1.3 添加测试

**创建测试文件**
```powershell
# 创建测试目录
New-Item -ItemType Directory -Path "tests/skills/$skillName"

# 创建测试文件
@"
// tests/skills/$skillName/test.cjs
const assert = require('assert');
const { execSync } = require('child_process');

describe('$skillName', () => {
  it('should execute successfully', () => {
    const result = execSync(
      'node .agents/skills/$skillName/execute.cjs arg1 arg2',
      { encoding: 'utf8' }
    );

    const output = JSON.parse(result);
    assert.strictEqual(output.success, true);
  });

  it('should handle errors gracefully', () => {
    try {
      execSync(
        'node .agents/skills/$skillName/execute.cjs invalid',
        { encoding: 'utf8' }
      );
      assert.fail('Should have thrown error');
    } catch (err) {
      assert.ok(err.message.includes('Error'));
    }
  });
});
"@ | Set-Content "tests/skills/$skillName/test.cjs"
```

**运行测试**
```powershell
npm test -- tests/skills/$skillName/test.cjs
```

---

## 【第二章：集成到工作流】

### 2.1 注册 Skill

**步骤 1：更新 sy-workflow 路由**

编辑 `.agents/skills/sy-workflow/SKILL.md`，添加新 Skill 的路由规则。

**步骤 2：添加到依赖图**

如果新 Skill 依赖其他 Skills，更新依赖关系：

```yaml
# .ai/workflow/skill-dependencies.yaml
skills:
  sy-example-skill:
    depends_on:
      - sy-constraints
      - sy-workflow
    required_phase: execute
```

### 2.2 配置权限

**步骤 1：定义允许的工具**

在 SKILL.md 的 `allowed-tools` 中明确列出：

```yaml
allowed-tools:
  - Read      # 读取文件
  - Write     # 写入文件
  - Edit      # 编辑文件
  - Bash      # 执行命令
  - Glob      # 文件匹配
  - Grep      # 内容搜索
```

**步骤 2：配置审批要求**

对于危险操作，设置 `requires-approval: true`：

```yaml
requires-approval: true  # 需要人工批准
```

### 2.3 添加 Hook 集成

**PreToolUse Hook 验证**

创建 Hook 验证 Skill 的前置条件：

```javascript
// scripts/hooks/sy-pretool-skill.cjs
const fs = require('fs');

const input = JSON.parse(fs.readFileSync(0, 'utf8'));

// 验证 Skill 调用的前置条件
if (input.tool_name === 'Skill' && input.tool_input.skill === 'sy-example-skill') {
  const state = loadWorkflowState();

  // 检查阶段
  if (state.phase.current !== 'execute') {
    console.log(JSON.stringify({
      continue: false,
      decision: 'deny',
      reason: 'sy-example-skill can only be used in execute phase',
      systemMessage: 'Please transition to execute phase first'
    }));
    process.exit(0);
  }

  // 检查依赖
  if (!state.skills_executed.includes('sy-constraints')) {
    console.log(JSON.stringify({
      continue: false,
      decision: 'deny',
      reason: 'sy-constraints must be executed first',
      systemMessage: 'Please run /sy-constraints before using this skill'
    }));
    process.exit(0);
  }
}

// 放行
console.log(JSON.stringify({ continue: true }));
```

---

## 【第三章：测试 Skills】

### 3.1 单元测试

**测试 Skill 脚本**

```javascript
// tests/skills/sy-example-skill/unit.test.cjs
const assert = require('assert');
const { executeSkillLogic } = require('../../../.agents/skills/sy-example-skill/execute.cjs');

describe('sy-example-skill unit tests', () => {
  it('should process input correctly', async () => {
    const result = await executeSkillLogic('arg1', 'arg2', {});
    assert.strictEqual(result.success, true);
  });

  it('should validate arguments', async () => {
    try {
      await executeSkillLogic(null, null, {});
      assert.fail('Should have thrown error');
    } catch (err) {
      assert.ok(err.message.includes('Invalid arguments'));
    }
  });
});
```

### 3.2 集成测试

**测试 Skill 与工作流集成**

```javascript
// tests/skills/sy-example-skill/integration.test.cjs
const assert = require('assert');
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

describe('sy-example-skill integration tests', () => {
  let testDir;

  beforeEach(() => {
    // 创建测试环境
    testDir = fs.mkdtempSync(path.join(require('os').tmpdir(), 'skill-test-'));
    process.chdir(testDir);

    // 初始化工作流状态
    fs.mkdirSync('.ai/workflow', { recursive: true });
    fs.writeFileSync('.ai/workflow/session.yaml', `
phase:
  current: execute
skills_executed:
  - sy-constraints
`);
  });

  afterEach(() => {
    // 清理测试环境
    fs.rmSync(testDir, { recursive: true, force: true });
  });

  it('should execute in correct phase', () => {
    const result = execSync(
      'node .agents/skills/sy-example-skill/execute.cjs arg1 arg2',
      { encoding: 'utf8' }
    );

    const output = JSON.parse(result);
    assert.strictEqual(output.success, true);
  });

  it('should fail in wrong phase', () => {
    // 修改阶段为 plan
    fs.writeFileSync('.ai/workflow/session.yaml', `
phase:
  current: plan
`);

    try {
      execSync(
        'node .agents/skills/sy-example-skill/execute.cjs arg1 arg2',
        { encoding: 'utf8' }
      );
      assert.fail('Should have failed');
    } catch (err) {
      assert.ok(err.message.includes('wrong phase'));
    }
  });
});
```

### 3.3 端到端测试

**测试完整工作流**

```javascript
// tests/skills/sy-example-skill/e2e.test.cjs
const assert = require('assert');
const { execSync } = require('child_process');

describe('sy-example-skill E2E tests', () => {
  it('should complete full workflow', function() {
    this.timeout(60000); // 60 秒超时

    // 1. 初始化工作流
    execSync('node scripts/runtime/init-workflow.cjs');

    // 2. 加载约束
    execSync('node .agents/skills/sy-constraints/execute.cjs');

    // 3. 转换到 execute 阶段
    execSync('node scripts/runtime/transition-phase.cjs execute');

    // 4. 执行 Skill
    const result = execSync(
      'node .agents/skills/sy-example-skill/execute.cjs arg1 arg2',
      { encoding: 'utf8' }
    );

    const output = JSON.parse(result);
    assert.strictEqual(output.success, true);

    // 5. 验证状态更新
    const state = require('.ai/workflow/session.yaml');
    assert.ok(state.skills_executed.includes('sy-example-skill'));
  });
});
```

---

## 【第四章：部署 Skills】

### 4.1 版本管理

**语义化版本**

```yaml
# .agents/skills/sy-example-skill/VERSION
version: 1.0.0
changelog:
  - version: 1.0.0
    date: 2026-03-12
    changes:
      - Initial release
      - Basic functionality
```

**Git 标签**

```powershell
git tag -a skills/sy-example-skill/v1.0.0 -m "Release sy-example-skill v1.0.0"
git push origin skills/sy-example-skill/v1.0.0
```

### 4.2 文档生成

**自动生成 Skills 文档**

```powershell
# scripts/generate-skills-docs.ps1
$skills = Get-ChildItem -Path ".agents/skills" -Directory

$doc = @"
# Skills 文档

## 可用 Skills

"@

foreach ($skill in $skills) {
    $skillMd = Get-Content "$($skill.FullName)/SKILL.md" -Raw
    $doc += "`n## $($skill.Name)`n`n"
    $doc += $skillMd
}

$doc | Set-Content "docs/skills-reference.md"
```

### 4.3 CI/CD 集成

**GitHub Actions 工作流**

```yaml
# .github/workflows/test-skills.yml
name: Test Skills

on:
  push:
    paths:
      - '.agents/skills/**'
  pull_request:
    paths:
      - '.agents/skills/**'

jobs:
  test:
    runs-on: windows-latest

    steps:
      - uses: actions/checkout@v3

      - uses: actions/setup-node@v3
        with:
          node-version: '18'

      - name: Install dependencies
        run: npm ci

      - name: Run skill tests
        run: npm test -- tests/skills/

      - name: Validate skill metadata
        run: node scripts/validate-skills.cjs

      - name: Generate documentation
        run: powershell.exe -File scripts/generate-skills-docs.ps1

      - name: Upload test results
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: test-results
          path: test-results/
```

---

## 【第五章：监控和维护】

### 5.1 Skill 使用统计

**记录 Skill 调用**

```javascript
// scripts/runtime/skill-tracker.cjs
const fs = require('fs');
const path = require('path');

function trackSkillUsage(skillName, args, result) {
  const logPath = path.join(process.cwd(), '.ai/workflow/skill-usage.jsonl');

  const entry = {
    timestamp: new Date().toISOString(),
    skill: skillName,
    args,
    success: result.success,
    duration_ms: result.duration_ms
  };

  fs.appendFileSync(logPath, JSON.stringify(entry) + '\n');
}

module.exports = { trackSkillUsage };
```

**生成使用报告**

```powershell
# scripts/skill-usage-report.ps1
$usage = Get-Content ".ai/workflow/skill-usage.jsonl" | ConvertFrom-Json

$report = $usage | Group-Object -Property skill | ForEach-Object {
    [PSCustomObject]@{
        Skill = $_.Name
        Count = $_.Count
        SuccessRate = ($_.Group | Where-Object { $_.success } | Measure-Object).Count / $_.Count * 100
        AvgDuration = ($_.Group | Measure-Object -Property duration_ms -Average).Average
    }
} | Sort-Object -Property Count -Descending

$report | Format-Table -AutoSize
```

### 5.2 性能监控

**Skill 性能基准**

```javascript
// tests/skills/benchmarks.cjs
const Benchmark = require('benchmark');
const suite = new Benchmark.Suite();

suite
  .add('sy-example-skill', () => {
    execSync('node .agents/skills/sy-example-skill/execute.cjs arg1 arg2');
  })
  .on('cycle', (event) => {
    console.log(String(event.target));
  })
  .on('complete', function() {
    console.log('Fastest is ' + this.filter('fastest').map('name'));
  })
  .run({ async: true });
```

### 5.3 错误追踪

**Skill 错误日志**

```javascript
// scripts/runtime/skill-error-handler.cjs
function handleSkillError(skillName, error) {
  const errorLog = {
    timestamp: new Date().toISOString(),
    skill: skillName,
    error: {
      message: error.message,
      stack: error.stack
    }
  };

  // 记录到 journal
  fs.appendFileSync(
    '.ai/workflow/journal.jsonl',
    JSON.stringify({ event: 'skill_error', ...errorLog }) + '\n'
  );

  // 记录到 Windows 事件日志
  if (process.platform === 'win32') {
    execSync(
      `powershell.exe -Command "Write-EventLog -LogName Application -Source 'seeyue-workflows' -EntryType Error -EventId 3001 -Message 'Skill error: ${skillName} - ${error.message}'"`
    );
  }
}

module.exports = { handleSkillError };
```

---

## 【第六章：最佳实践】

### 6.1 Skill 设计原则

**1. 单一职责**
```
✅ 好：sy-test-runner - 仅运行测试
❌ 坏：sy-test-and-deploy - 运行测试并部署
```

**2. 可组合性**
```
✅ 好：sy-ideation + sy-design + sy-writing-plans
❌ 坏：sy-plan-everything（单体 Skill）
```

**3. 幂等性**
```javascript
// ✅ 好：多次执行结果一致
function createCheckpoint(label) {
  if (checkpointExists(label)) {
    return existingCheckpoint(label);
  }
  return newCheckpoint(label);
}

// ❌ 坏：多次执行产生不同结果
function createCheckpoint(label) {
  return newCheckpoint(label + Date.now());
}
```

**4. 错误处理**
```javascript
// ✅ 好：详细的错误信息
throw new Error(`Failed to execute task-001: File not found - src/module.js`);

// ❌ 坏：模糊的错误信息
throw new Error('Something went wrong');
```

### 6.2 性能优化

**1. 缓存结果**
```javascript
const cache = new Map();

function expensiveOperation(input) {
  if (cache.has(input)) {
    return cache.get(input);
  }

  const result = doExpensiveOperation(input);
  cache.set(input, result);
  return result;
}
```

**2. 并行执行**
```javascript
// ✅ 好：并行执行独立任务
const results = await Promise.all([
  executeTask1(),
  executeTask2(),
  executeTask3()
]);

// ❌ 坏：串行执行独立任务
const result1 = await executeTask1();
const result2 = await executeTask2();
const result3 = await executeTask3();
```

**3. 增量处理**
```javascript
// ✅ 好：仅处理变更的文件
const changedFiles = getChangedFiles();
for (const file of changedFiles) {
  processFile(file);
}

// ❌ 坏：处理所有文件
const allFiles = getAllFiles();
for (const file of allFiles) {
  processFile(file);
}
```

### 6.3 安全考虑

**1. 输入验证**
```javascript
function validateInput(args) {
  if (!args.taskId) {
    throw new Error('taskId is required');
  }

  if (!/^task-\d+$/.test(args.taskId)) {
    throw new Error('Invalid taskId format');
  }

  return true;
}
```

**2. 路径安全**
```javascript
const path = require('path');

function safeReadFile(filePath) {
  // 防止路径遍历攻击
  const safePath = path.resolve(process.cwd(), filePath);

  if (!safePath.startsWith(process.cwd())) {
    throw new Error('Access denied: Path outside workspace');
  }

  return fs.readFileSync(safePath, 'utf8');
}
```

**3. 命令注入防护**
```javascript
const { execFileSync } = require('child_process');

// ✅ 好：使用 execFileSync，参数分离
execFileSync('git', ['commit', '-m', userMessage]);

// ❌ 坏：使用 exec，字符串拼接
exec(`git commit -m "${userMessage}"`); // 可能被注入
```

---

## 【附录】

### A. Skill 模板

**基础 Skill 模板**：
```
.agents/skills/sy-template/
├── SKILL.md          # 元数据和文档
├── execute.cjs       # 执行脚本（可选）
├── README.md         # 详细说明
└── examples/         # 使用示例
    └── example1.md
```

### B. 常用工具函数

```javascript
// scripts/runtime/skill-utils.cjs

// 读取工作流状态
function readWorkflowState() {
  const yaml = require('js-yaml');
  const content = fs.readFileSync('.ai/workflow/session.yaml', 'utf8');
  return yaml.load(content);
}

// 更新工作流状态
function updateWorkflowState(updates) {
  const state = readWorkflowState();
  const newState = { ...state, ...updates };
  const yaml = require('js-yaml');
  fs.writeFileSync('.ai/workflow/session.yaml', yaml.dump(newState));
}

// 记录到 journal
function logToJournal(event) {
  fs.appendFileSync(
    '.ai/workflow/journal.jsonl',
    JSON.stringify({ timestamp: new Date().toISOString(), ...event }) + '\n'
  );
}

module.exports = {
  readWorkflowState,
  updateWorkflowState,
  logToJournal
};
```

---

**文档版本**：v1.0.0
**最后更新**：2026-03-12
**作者**：seeyue-workflows 架构团队
