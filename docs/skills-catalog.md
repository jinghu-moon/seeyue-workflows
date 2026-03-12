# seeyue-workflows Skills 完整目录

## 【文档定位】

本文档提供 seeyue-workflows 所有 Skills 的完整目录和使用指南。

**相关文档**：
- `skills-architecture.md` - 架构设计
- `skills-development-workflow.md` - 开发流程
- `skills-implementation-guide.md` - 实施指南

---

## 【第一章：现有 Skills（17 个）】

### 1.1 编排层 Skills

#### sy-workflow
**触发条件**：统一工作流入口

**功能**：
- 路由到正确的阶段 Skill
- 管理工作流状态转换
- 协调多个 Skills 执行

**参数**：
```bash
/sy-workflow [phase] [action]
```

**示例**：
```bash
/sy-workflow plan start
/sy-workflow execute continue
/sy-workflow review request
```

**元数据**：
```yaml
category: orchestration
phase: all
allowed-tools: [Read, Write, Bash]
requires-approval: false
```

---

### 1.2 计划层 Skills

#### sy-ideation
**触发条件**：需要将模糊需求转化为可批准的设计

**功能**：
- 分析用户需求
- 识别关键功能点
- 生成初步设计方案
- 评估技术可行性

**参数**：
```bash
/sy-ideation [requirement_description]
```

**输出**：
- 需求分析文档（`.ai/specs/ideation.md`）
- 功能清单
- 技术选型建议

**元数据**：
```yaml
category: planning
phase: plan
allowed-tools: [Read, Write, WebSearch]
requires-approval: true
```

---

#### sy-design
**触发条件**：将上游输出收敛为批准的技术架构

**功能**：
- 细化技术架构
- 定义接口和数据模型
- 识别技术风险
- 生成设计文档

**参数**：
```bash
/sy-design [ideation_doc]
```

**输出**：
- 技术设计文档（`.ai/specs/design.md`）
- 架构图
- API 规范
- 数据模型

**元数据**：
```yaml
category: planning
phase: plan
allowed-tools: [Read, Write]
requires-approval: true
dependencies: [sy-ideation]
```

---

#### sy-writing-plans
**触发条件**：将批准的设计转化为可执行步骤

**功能**：
- 任务分解
- 依赖关系分析
- 优先级排序
- 生成执行计划

**参数**：
```bash
/sy-writing-plans [design_doc]
```

**输出**：
- 执行计划（`.ai/plans/execution-plan.yaml`）
- 任务列表
- 依赖图
- 时间估算

**元数据**：
```yaml
category: planning
phase: plan
allowed-tools: [Read, Write]
requires-approval: true
dependencies: [sy-design]
```

---

#### sy-code-insight
**触发条件**：需要构建代码库理解工件

**功能**：
- 分析现有代码结构
- 识别关键模块
- 生成代码地图
- 提取设计模式

**参数**：
```bash
/sy-code-insight [target_directory]
```

**输出**：
- 代码分析报告（`.ai/insights/code-analysis.md`）
- 模块依赖图
- 复杂度分析
- 重构建议

**元数据**：
```yaml
category: planning
phase: plan
allowed-tools: [Read, Glob, Grep]
requires-approval: false
```

---

### 1.3 执行层 Skills

#### sy-executing-plans
**触发条件**：按节点执行批准的计划

**功能**：
- 读取执行计划
- 按顺序执行任务
- 记录执行证据
- 更新任务状态

**参数**：
```bash
/sy-executing-plans [task_id]
```

**执行流程**：
1. 读取任务详情
2. 验证前置条件（TDD Red Gate）
3. 执行任务步骤
4. 运行测试验证
5. 记录证据到 journal
6. 更新状态

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Read, Write, Edit, Bash]
requires-approval: false
dependencies: [sy-writing-plans, sy-constraints]
```

---

#### sy-debug
**触发条件**：需要进行根本原因调查

**功能**：
- 分析错误日志
- 定位问题根源
- 生成修复方案
- 验证修复效果

**参数**：
```bash
/sy-debug [error_description]
```

**调试流程**：
1. 收集错误信息
2. 重现问题
3. 分析根本原因
4. 提出修复方案
5. 验证修复

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Read, Bash, Grep]
requires-approval: false
```

---

#### sy-verification-before-completion
**触发条件**：完成前的完整验证

**功能**：
- 运行所有测试
- 检查代码覆盖率
- 扫描安全漏洞
- 验证文档完整性
- 生成验证报告

**参数**：
```bash
/sy-verification-before-completion
```

**验证清单**：
- ✅ 所有测试通过
- ✅ 代码覆盖率 > 80%
- ✅ 无安全漏洞
- ✅ 无占位符代码
- ✅ 文档已更新
- ✅ CHANGELOG 已记录

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Read, Bash]
requires-approval: false
```

---

### 1.4 协作层 Skills

#### sy-requesting-code-review
**触发条件**：代码审查前检查清单

**功能**：
- 验证代码质量
- 生成审查请求
- 准备审查材料
- 通知审查者

**参数**：
```bash
/sy-requesting-code-review [reviewer]
```

**审查前检查**：
- 代码格式化
- Lint 检查通过
- 测试覆盖率达标
- 文档完整
- CHANGELOG 更新

**元数据**：
```yaml
category: collaboration
phase: review
allowed-tools: [Read, Write, Bash]
requires-approval: false
```

---

#### sy-receiving-code-review
**触发条件**：处理审查反馈

**功能**：
- 解析审查意见
- 分类反馈（必须修改/建议/讨论）
- 生成修复计划
- 跟踪修复进度

**参数**：
```bash
/sy-receiving-code-review [review_comments]
```

**处理流程**：
1. 解析审查意见
2. 分类和优先级排序
3. 生成修复任务
4. 执行修复
5. 回复审查者

**元数据**：
```yaml
category: collaboration
phase: review
allowed-tools: [Read, Write, Edit]
requires-approval: false
```

---

#### sy-doc-sync
**触发条件**：记录、合并和审查结构化变更日志

**功能**：
- 同步代码和文档
- 生成 API 文档
- 更新 README
- 维护 CHANGELOG

**参数**：
```bash
/sy-doc-sync [scope]
```

**同步范围**：
- `api`: API 文档
- `readme`: README 文件
- `changelog`: CHANGELOG
- `all`: 所有文档

**元数据**：
```yaml
category: collaboration
phase: execute
allowed-tools: [Read, Write]
requires-approval: false
```

---

### 1.5 约束层 Skills

#### sy-constraints
**触发条件**：加载工作流约束和硬守卫

**功能**：
- 加载 12 个子约束
- 验证约束满足
- 提供约束建议
- 记录约束违规

**子约束列表**：
1. `sy-constraints/language` - 语言分区 + RFC 关键字
2. `sy-constraints/truth` - 零幻觉 + 证据优先
3. `sy-constraints/execution` - 真实来源 + 阶段门
4. `sy-constraints/research` - 重用优先搜索
5. `sy-constraints/debug` - 根本原因优先
6. `sy-constraints/review` - 审查反馈协议
7. `sy-constraints/verify` - 完成验证矩阵
8. `sy-constraints/workspace` - 工作区隔离
9. `sy-constraints/appsec` - 应用安全护栏
10. `sy-constraints/safety` - 高风险操作守卫
11. `sy-constraints/testing` - TDD + 反模式门
12. `sy-constraints/phase` - DAG 顺序 + 检查点

**参数**：
```bash
/sy-constraints [constraint_name]
```

**元数据**：
```yaml
category: constraints
phase: all
allowed-tools: [Read]
requires-approval: false
```

---

### 1.6 工具层 Skills

#### sy-worktree
**触发条件**：准备隔离工作树和基线检查

**功能**：
- 创建 Git worktree
- 隔离实验性变更
- 基线健康检查
- 清理 worktree

**参数**：
```bash
/sy-worktree create [branch_name]
/sy-worktree cleanup
```

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Bash]
requires-approval: false
```

---

#### sy-changelog
**触发条件**：变更日志管理

**功能**：
- 记录变更
- 分类变更（feat/fix/docs/refactor）
- 生成版本日志
- 发布说明

**参数**：
```bash
/sy-changelog add [type] [description]
/sy-changelog generate [version]
```

**元数据**：
```yaml
category: collaboration
phase: execute
allowed-tools: [Read, Write]
requires-approval: false
```

---

#### sy-git-commit
**触发条件**：Git 提交管理

**功能**：
- 生成符合规范的提交消息
- 验证提交内容
- 执行提交
- 推送到远程

**参数**：
```bash
/sy-git-commit [message]
```

**提交消息格式**：
```
<type>(<scope>): <subject>

<body>

<footer>
```

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Bash]
requires-approval: true
```

---

## 【第二章：建议新增 Skills（8 个）】

### 2.1 需求阶段 Skills

#### sy-requirements-analysis（新增）
**触发条件**：需要深度分析用户需求

**功能**：
- 需求提取和分类
- 用户故事生成
- 验收标准定义
- 需求优先级排序

**参数**：
```bash
/sy-requirements-analysis [input_source]
```

**输出**：
- 需求文档（`.ai/specs/requirements.md`）
- 用户故事列表
- 验收标准
- 优先级矩阵

**元数据**：
```yaml
category: planning
phase: plan
allowed-tools: [Read, Write, WebSearch]
requires-approval: true
```

---

### 2.2 测试阶段 Skills

#### sy-test-planning（新增）
**触发条件**：需要制定测试策略

**功能**：
- 测试用例设计
- 测试数据准备
- 测试环境配置
- 测试计划生成

**参数**：
```bash
/sy-test-planning [design_doc]
```

**输出**：
- 测试计划（`.ai/tests/test-plan.md`）
- 测试用例列表
- 测试数据
- 环境配置

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Read, Write]
requires-approval: false
```

---

#### sy-test-execution（新增）
**触发条件**：执行测试并收集结果

**功能**：
- 运行单元测试
- 运行集成测试
- 收集测试报告
- 分析测试覆盖率

**参数**：
```bash
/sy-test-execution [test_type]
```

**测试类型**：
- `unit`: 单元测试
- `integration`: 集成测试
- `e2e`: 端到端测试
- `all`: 所有测试

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Bash, Read]
requires-approval: false
```

---

### 2.3 部署阶段 Skills

#### sy-deployment-preparation（新增）
**触发条件**：准备部署

**功能**：
- 构建检查
- 依赖验证
- 配置文件准备
- 部署清单生成

**参数**：
```bash
/sy-deployment-preparation [environment]
```

**环境**：
- `dev`: 开发环境
- `staging`: 预发布环境
- `production`: 生产环境

**元数据**：
```yaml
category: execution
phase: done
allowed-tools: [Read, Bash]
requires-approval: true
```

---

#### sy-deployment-execution（新增）
**触发条件**：执行部署

**功能**：
- 执行部署脚本
- 健康检查
- 回滚准备
- 部署验证

**参数**：
```bash
/sy-deployment-execution [environment] [version]
```

**元数据**：
```yaml
category: execution
phase: done
allowed-tools: [Bash]
requires-approval: true
```

---

### 2.4 维护阶段 Skills

#### sy-monitoring-setup（新增）
**触发条件**：设置监控和告警

**功能**：
- 配置监控指标
- 设置告警规则
- 日志聚合配置
- 性能基线建立

**参数**：
```bash
/sy-monitoring-setup [service_name]
```

**元数据**：
```yaml
category: execution
phase: done
allowed-tools: [Read, Write, Bash]
requires-approval: false
```

---

#### sy-incident-response（新增）
**触发条件**：响应生产事故

**功能**：
- 事故分类和优先级
- 快速诊断
- 临时修复
- 事故报告生成

**参数**：
```bash
/sy-incident-response [incident_id]
```

**响应流程**：
1. 事故确认和分类
2. 影响范围评估
3. 快速诊断
4. 临时修复或回滚
5. 根本原因分析
6. 永久修复计划
7. 事故报告

**元数据**：
```yaml
category: execution
phase: done
allowed-tools: [Read, Bash, WebSearch]
requires-approval: false
```

---

#### sy-refactoring（新增）
**触发条件**：代码重构

**功能**：
- 识别重构机会
- 生成重构计划
- 执行重构
- 验证重构结果

**参数**：
```bash
/sy-refactoring [target_module]
```

**重构类型**：
- 提取方法
- 提取类
- 移动方法
- 重命名
- 简化条件表达式

**元数据**：
```yaml
category: execution
phase: execute
allowed-tools: [Read, Write, Edit, Bash]
requires-approval: false
```

---

## 【第三章：Skills 使用指南】

### 3.1 典型工作流

**完整开发流程**：
```bash
# 1. 需求分析
/sy-requirements-analysis "用户需要一个登录功能"

# 2. 概念设计
/sy-ideation requirements.md

# 3. 技术设计
/sy-design ideation.md

# 4. 执行计划
/sy-writing-plans design.md

# 5. 代码实现
/sy-executing-plans task-001

# 6. 测试
/sy-test-execution unit

# 7. 验证
/sy-verification-before-completion

# 8. 代码审查
/sy-requesting-code-review @reviewer

# 9. 处理反馈
/sy-receiving-code-review review-comments.md

# 10. 部署
/sy-deployment-preparation production
/sy-deployment-execution production v1.0.0
```

### 3.2 快速参考

**按阶段查找 Skills**：

| 阶段 | Skills |
|------|--------|
| **Plan** | sy-requirements-analysis, sy-ideation, sy-design, sy-writing-plans, sy-code-insight |
| **Execute** | sy-executing-plans, sy-debug, sy-test-planning, sy-test-execution, sy-verification-before-completion, sy-refactoring |
| **Review** | sy-requesting-code-review, sy-receiving-code-review |
| **Done** | sy-deployment-preparation, sy-deployment-execution, sy-monitoring-setup |
| **All** | sy-workflow, sy-constraints, sy-doc-sync, sy-changelog, sy-git-commit |

**按功能查找 Skills**：

| 功能 | Skills |
|------|--------|
| **需求分析** | sy-requirements-analysis, sy-ideation |
| **设计** | sy-design, sy-code-insight |
| **计划** | sy-writing-plans, sy-test-planning |
| **实现** | sy-executing-plans, sy-refactoring |
| **测试** | sy-test-planning, sy-test-execution, sy-verification-before-completion |
| **调试** | sy-debug, sy-incident-response |
| **审查** | sy-requesting-code-review, sy-receiving-code-review |
| **文档** | sy-doc-sync, sy-changelog |
| **部署** | sy-deployment-preparation, sy-deployment-execution |
| **监控** | sy-monitoring-setup, sy-incident-response |

### 3.3 Skills 组合模式

**模式 1：快速原型**
```bash
/sy-ideation "快速原型需求"
/sy-executing-plans prototype-task
/sy-verification-before-completion
```

**模式 2：完整功能开发**
```bash
/sy-requirements-analysis "完整需求文档"
/sy-ideation requirements.md
/sy-design ideation.md
/sy-writing-plans design.md
/sy-test-planning design.md
/sy-executing-plans task-001
/sy-test-execution all
/sy-verification-before-completion
/sy-requesting-code-review @reviewer
```

**模式 3：Bug 修复**
```bash
/sy-debug "错误描述"
/sy-executing-plans bugfix-task
/sy-test-execution unit
/sy-verification-before-completion
```

**模式 4：重构**
```bash
/sy-code-insight target-module
/sy-refactoring target-module
/sy-test-execution all
/sy-verification-before-completion
```

---

## 【第四章：Skills 扩展】

### 4.1 创建自定义 Skill

**步骤**：
1. 在 `.agents/skills/` 创建目录
2. 编写 `SKILL.md` 元数据
3. 实现 Skill 逻辑
4. 添加测试
5. 更新文档

**模板**：
```markdown
---
name: sy-custom-skill
description: Use when [condition] - [function]
category: execution
phase: execute
allowed-tools: [Read, Write]
argument-hint: "[arg1]"
disable-model-invocation: false
requires-approval: false
dependencies: []
---

# sy-custom-skill

## 触发条件
[描述何时使用此 Skill]

## 功能
[详细功能说明]

## 参数
[参数列表和说明]

## 输出
[输出格式和内容]

## 示例
\`\`\`bash
/sy-custom-skill example-arg
\`\`\`
```

### 4.2 Skill 测试

**测试文件位置**：
```
tests/skills/
├── sy-custom-skill.test.cjs
└── fixtures/
    └── sy-custom-skill/
        ├── input.json
        └── expected-output.json
```

**测试示例**：
```javascript
const { execSync } = require('child_process');
const assert = require('assert');

describe('sy-custom-skill', () => {
  it('should execute successfully', () => {
    const result = execSync(
      'node .agents/skills/sy-custom-skill/index.cjs arg1',
      { encoding: 'utf8' }
    );

    assert.ok(result.includes('Success'));
  });
});
```

---

**文档版本**：v1.0.0
**最后更新**：2026-03-12
**维护者**：seeyue-workflows 团队
