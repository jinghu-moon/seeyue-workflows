# seeyue-workflows 开发流程 Skills 方案

## 【文档定位】

本文档定义完整的项目开发流程 Skills，覆盖从需求到部署的全生命周期。

**相关文档**：
- `skills-architecture.md` - 架构设计
- `skills-catalog.md` - Skills 目录
- `skills-implementation-guide.md` - 实施指南

---

## 【第一章：开发流程概览】

### 1.1 完整生命周期

```
需求阶段 (Requirements)
    ↓
设计阶段 (Design)
    ↓
开发阶段 (Development)
    ↓
测试阶段 (Testing)
    ↓
审查阶段 (Review)
    ↓
部署阶段 (Deployment)
    ↓
维护阶段 (Maintenance)
```

### 1.2 阶段与 Skills 映射

| 阶段 | V4 Phase | 核心 Skills | 输出工件 |
|------|----------|------------|---------|
| 需求 | plan | sy-ideation, sy-requirements-analysis | 需求文档 |
| 设计 | plan | sy-design, sy-code-insight | 设计文档 |
| 开发 | execute | sy-executing-plans, sy-debug | 代码 + 测试 |
| 测试 | execute | sy-verification-before-completion | 测试报告 |
| 审查 | review | sy-requesting-code-review | 审查报告 |
| 部署 | done | sy-deployment-automation | 部署日志 |
| 维护 | done | sy-monitoring-alerts | 监控数据 |

---

## 【第二章：需求阶段 Skills】

### 2.1 sy-ideation（现有）

**工作流程**：
```
用户输入需求描述
    ↓
sy-ideation 分析需求
    ↓
生成功能清单
    ↓
评估技术可行性
    ↓
输出需求文档
    ↓
请求批准
```

**输入示例**：
```bash
/sy-ideation "构建一个用户认证系统，支持邮箱登录和 OAuth"
```

**输出工件**：
```markdown
# 需求分析文档

## 功能需求
1. 用户注册（邮箱 + 密码）
2. 用户登录（邮箱 + 密码）
3. OAuth 集成（Google、GitHub）
4. 密码重置
5. 会话管理

## 非功能需求
- 安全性：密码加密、HTTPS
- 性能：登录响应 < 500ms
- 可用性：99.9% uptime

## 技术选型建议
- 后端：Node.js + Express
- 数据库：PostgreSQL
- 认证：Passport.js
- OAuth：passport-google-oauth20

## 风险评估
- OAuth 配置复杂度：中
- 密码安全合规性：高
- 会话管理复杂度：中
```

### 2.2 sy-requirements-analysis（建议新增）

**功能增强**：
- 用户故事生成
- 验收标准定义
- 优先级排序
- 依赖关系分析

**输入示例**：
```bash
/sy-requirements-analysis .ai/specs/ideation.md
```

**输出工件**：
```yaml
# 用户故事

stories:
  - id: US-001
    title: 用户注册
    as_a: 新用户
    i_want: 使用邮箱注册账号
    so_that: 可以访问系统功能
    acceptance_criteria:
      - 邮箱格式验证
      - 密码强度检查（8+ 字符，包含数字和字母）
      - 注册成功后自动登录
      - 发送欢迎邮件
    priority: high
    estimate: 3 points

  - id: US-002
    title: OAuth 登录
    as_a: 用户
    i_want: 使用 Google 账号登录
    so_that: 无需记住额外密码
    acceptance_criteria:
      - 点击 "使用 Google 登录" 按钮
      - 跳转到 Google 授权页面
      - 授权成功后返回系统
      - 自动创建或关联账号
    priority: medium
    estimate: 5 points
    dependencies: [US-001]
```

---

## 【第三章：设计阶段 Skills】

### 3.1 sy-design（现有）

**工作流程**：
```
读取需求文档
    ↓
sy-design 生成技术设计
    ↓
定义架构和接口
    ↓
识别技术风险
    ↓
输出设计文档
    ↓
请求批准
```

**输入示例**：
```bash
/sy-design .ai/specs/ideation.md
```

**输出工件**：
```markdown
# 技术设计文档

## 系统架构

\`\`\`
┌─────────────┐
│   Client    │
│  (React)    │
└──────┬──────┘
       │ HTTPS
┌──────▼──────┐
│   API       │
│  (Express)  │
└──────┬──────┘
       │
┌──────▼──────┐
│  Database   │
│ (PostgreSQL)│
└─────────────┘
\`\`\`

## API 设计

### POST /api/auth/register
**请求**：
\`\`\`json
{
  "email": "user@example.com",
  "password": "SecurePass123"
}
\`\`\`

**响应**：
\`\`\`json
{
  "user": {
    "id": "uuid",
    "email": "user@example.com"
  },
  "token": "jwt-token"
}
\`\`\`

### POST /api/auth/login
### GET /api/auth/oauth/google
### POST /api/auth/reset-password

## 数据模型

\`\`\`sql
CREATE TABLE users (
  id UUID PRIMARY KEY,
  email VARCHAR(255) UNIQUE NOT NULL,
  password_hash VARCHAR(255),
  oauth_provider VARCHAR(50),
  oauth_id VARCHAR(255),
  created_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE sessions (
  id UUID PRIMARY KEY,
  user_id UUID REFERENCES users(id),
  token VARCHAR(255) UNIQUE NOT NULL,
  expires_at TIMESTAMP NOT NULL
);
\`\`\`

## 安全设计
- 密码使用 bcrypt 加密（cost factor: 12）
- JWT token 有效期：24 小时
- HTTPS 强制
- CSRF 保护
- Rate limiting：5 次/分钟

## 技术风险
1. OAuth 回调 URL 配置错误 → 测试环境验证
2. 密码哈希性能问题 → 异步处理
3. 会话存储扩展性 → 使用 Redis
```

### 3.2 sy-code-insight（现有）

**功能**：分析现有代码库，识别集成点

**输入示例**：
```bash
/sy-code-insight src/
```

**输出工件**：
```markdown
# 代码库分析报告

## 现有模块
- `src/server.js` - Express 服务器入口
- `src/routes/` - 路由定义
- `src/models/` - 数据模型
- `src/middleware/` - 中间件

## 集成点
1. 在 `src/routes/` 下创建 `auth.js`
2. 在 `src/models/` 下创建 `User.js` 和 `Session.js`
3. 在 `src/middleware/` 下创建 `authenticate.js`

## 依赖分析
- 需要安装：`passport`, `passport-google-oauth20`, `bcrypt`, `jsonwebtoken`
- 现有依赖可复用：`express`, `pg`

## 代码风格
- 使用 ES6+ 语法
- 异步操作使用 async/await
- 错误处理使用 try-catch
```

---

## 【第四章：开发阶段 Skills】

### 4.1 sy-writing-plans（现有）

**工作流程**：
```
读取设计文档
    ↓
sy-writing-plans 生成执行计划
    ↓
任务分解
    ↓
依赖关系分析
    ↓
输出执行计划
    ↓
请求批准
```

**输入示例**：
```bash
/sy-writing-plans .ai/specs/design.md
```

**输出工件**：
```yaml
# 执行计划

plan:
  name: "用户认证系统实现"
  version: "1.0"
  created_at: "2026-03-12T10:00:00Z"

tasks:
  - id: task-001
    title: "创建 User 数据模型"
    description: "实现 User 模型，包含邮箱、密码哈希、OAuth 字段"
    dependencies: []
    estimate: 1h
    priority: high
    files:
      - src/models/User.js
    tests:
      - tests/models/User.test.js

  - id: task-002
    title: "实现密码加密"
    description: "使用 bcrypt 加密密码"
    dependencies: [task-001]
    estimate: 0.5h
    priority: high
    files:
      - src/utils/password.js
    tests:
      - tests/utils/password.test.js

  - id: task-003
    title: "实现注册 API"
    description: "POST /api/auth/register 端点"
    dependencies: [task-001, task-002]
    estimate: 2h
    priority: high
    files:
      - src/routes/auth.js
      - src/controllers/authController.js
    tests:
      - tests/routes/auth.test.js

  - id: task-004
    title: "实现登录 API"
    description: "POST /api/auth/login 端点"
    dependencies: [task-001, task-002]
    estimate: 1.5h
    priority: high

  - id: task-005
    title: "实现 OAuth Google 集成"
    description: "GET /api/auth/oauth/google 端点"
    dependencies: [task-001]
    estimate: 3h
    priority: medium

execution_order:
  - [task-001]
  - [task-002]
  - [task-003, task-004]
  - [task-005]
```

### 4.2 sy-executing-plans（现有）

**工作流程**：
```
读取执行计划
    ↓
选择下一个任务
    ↓
验证 TDD Red Gate
    ↓
编写失败测试
    ↓
实现功能代码
    ↓
运行测试验证
    ↓
记录证据
    ↓
更新任务状态
```

**TDD 流程**：
```
1. RED（写失败测试）
   ↓
2. 运行测试，确认失败
   ↓
3. GREEN（实现功能）
   ↓
4. 运行测试，确认通过
   ↓
5. REFACTOR（重构优化）
   ↓
6. 运行测试，确认仍通过
```

**输入示例**：
```bash
/sy-executing-plans task-001
```

**执行日志**：
```json
{
  "task_id": "task-001",
  "phase": "red",
  "timestamp": "2026-03-12T10:30:00Z",
  "action": "write_test",
  "file": "tests/models/User.test.js",
  "test_output": "FAIL: 1 test failed",
  "evidence": "journal_entry_001"
}

{
  "task_id": "task-001",
  "phase": "green",
  "timestamp": "2026-03-12T10:45:00Z",
  "action": "implement_feature",
  "file": "src/models/User.js",
  "test_output": "PASS: 1 test passed",
  "evidence": "journal_entry_002"
}
```

### 4.3 sy-debug（现有）

**触发场景**：
- 测试失败
- 运行时错误
- 性能问题
- 集成问题

**调试流程**：
```
收集错误信息
    ↓
重现问题
    ↓
分析根本原因
    ↓
提出修复方案
    ↓
实施修复
    ↓
验证修复效果
```

**输入示例**：
```bash
/sy-debug "登录 API 返回 500 错误"
```

**调试报告**：
```markdown
# 调试报告

## 问题描述
登录 API (`POST /api/auth/login`) 返回 500 Internal Server Error

## 错误信息
\`\`\`
Error: Cannot read property 'password_hash' of undefined
  at authController.login (src/controllers/authController.js:25)
\`\`\`

## 根本原因
数据库查询未找到用户时，代码尝试访问 `undefined.password_hash`

## 修复方案
在访问 `password_hash` 前检查用户是否存在：

\`\`\`javascript
// 修复前
const isValid = await bcrypt.compare(password, user.password_hash);

// 修复后
if (!user) {
  return res.status(401).json({ error: 'Invalid credentials' });
}
const isValid = await bcrypt.compare(password, user.password_hash);
\`\`\`

## 验证
- 测试用例：登录不存在的用户
- 预期结果：返回 401 Unauthorized
- 实际结果：✅ 通过
```

---

## 【第五章：测试阶段 Skills】

### 5.1 sy-verification-before-completion（现有）

**验证清单**：
```yaml
verification_checklist:
  code_quality:
    - name: "所有测试通过"
      command: "npm test"
      status: pending

    - name: "代码覆盖率 > 80%"
      command: "npm run coverage"
      threshold: 80
      status: pending

    - name: "Lint 检查通过"
      command: "npm run lint"
      status: pending

  security:
    - name: "无安全漏洞"
      command: "npm audit"
      status: pending

    - name: "无敏感信息泄露"
      command: "node scripts/scan-secrets.js"
      status: pending

  documentation:
    - name: "API 文档已更新"
      files: ["docs/api.md"]
      status: pending

    - name: "CHANGELOG 已记录"
      files: ["CHANGELOG.md"]
      status: pending

  completeness:
    - name: "无 TODO/FIXME"
      command: "grep -r 'TODO\\|FIXME' src/"
      expect_empty: true
      status: pending

    - name: "无占位符代码"
      command: "node scripts/detect-placeholders.js"
      status: pending
```

**执行示例**：
```bash
/sy-verification-before-completion
```

**验证报告**：
```markdown
# 验证报告

## 代码质量 ✅
- ✅ 所有测试通过 (15/15)
- ✅ 代码覆盖率 95% (> 80%)
- ✅ Lint 检查通过

## 安全性 ✅
- ✅ 无安全漏洞
- ✅ 无敏感信息泄露

## 文档 ✅
- ✅ API 文档已更新
- ✅ CHANGELOG 已记录

## 完整性 ✅
- ✅ 无 TODO/FIXME
- ✅ 无占位符代码

## 总体评估
**状态**: PASS
**可以进入审查阶段**: YES
```

---

## 【第六章：审查阶段 Skills】

### 6.1 sy-requesting-code-review（现有）

**审查前准备**：
```yaml
pre_review_checklist:
  - 代码格式化完成
  - 所有测试通过
  - 文档已更新
  - CHANGELOG 已记录
  - 分支已推送到远程
```

**输入示例**：
```bash
/sy-requesting-code-review @reviewer-name
```

**审查请求**：
```markdown
# 代码审查请求

## 变更概述
实现用户认证系统，包括：
- 用户注册（邮箱 + 密码）
- 用户登录
- OAuth Google 集成

## 变更文件
- `src/models/User.js` (+120 lines)
- `src/routes/auth.js` (+85 lines)
- `src/controllers/authController.js` (+150 lines)
- `src/middleware/authenticate.js` (+45 lines)
- `tests/` (+200 lines)

## 测试覆盖
- 单元测试：15 个
- 集成测试：5 个
- 覆盖率：95%

## 审查重点
1. 密码加密实现是否安全
2. OAuth 流程是否正确
3. 错误处理是否完善
4. API 设计是否合理

## 相关文档
- 设计文档：`.ai/specs/design.md`
- API 文档：`docs/api.md`
```

### 6.2 sy-receiving-code-review（现有）

**审查反馈处理**：
```yaml
review_feedback:
  - id: comment-001
    type: must_fix
    file: src/controllers/authController.js
    line: 45
    comment: "密码比较应该使用常量时间算法，防止时序攻击"
    status: pending

  - id: comment-002
    type: suggestion
    file: src/routes/auth.js
    line: 20
    comment: "建议添加 rate limiting"
    status: pending

  - id: comment-003
    type: question
    file: src/models/User.js
    line: 30
    comment: "为什么使用 UUID 而不是自增 ID？"
    status: pending
```

**处理流程**：
```
解析审查意见
    ↓
分类（必须修改/建议/讨论）
    ↓
生成修复任务
    ↓
执行修复
    ↓
回复审查者
    ↓
请求重新审查
```

---

## 【第七章：部署阶段 Skills】

### 7.1 sy-deployment-automation（建议新增）

**部署流程**：
```
验证部署前提条件
    ↓
构建生产版本
    ↓
运行部署前测试
    ↓
执行数据库迁移
    ↓
部署到目标环境
    ↓
运行冒烟测试
    ↓
记录部署日志
```

**输入示例**：
```bash
/sy-deployment-automation production
```

**部署脚本**：
```yaml
deployment:
  environment: production
  steps:
    - name: "验证前提条件"
      checks:
        - git_branch: main
        - tests_passing: true
        - code_review_approved: true

    - name: "构建"
      command: "npm run build"

    - name: "数据库迁移"
      command: "npm run migrate"

    - name: "部署"
      command: "npm run deploy:prod"

    - name: "冒烟测试"
      command: "npm run test:smoke"

  rollback_plan:
    - "npm run deploy:rollback"
    - "npm run migrate:rollback"
```

---

## 【第八章：维护阶段 Skills】

### 8.1 sy-monitoring-alerts（建议新增）

**监控指标**：
```yaml
monitoring:
  performance:
    - metric: response_time
      threshold: 500ms
      alert: warning

    - metric: error_rate
      threshold: 1%
      alert: critical

  availability:
    - metric: uptime
      threshold: 99.9%
      alert: critical

  security:
    - metric: failed_login_attempts
      threshold: 10/minute
      alert: warning
```

**告警处理**：
```
接收告警
    ↓
分析根本原因
    ↓
生成修复方案
    ↓
执行修复
    ↓
验证修复效果
    ↓
更新监控规则
```

---

## 【第九章：完整流程示例】

### 9.1 端到端示例

**需求**：实现用户认证系统

**完整流程**：

```bash
# 1. 需求阶段
/sy-ideation "构建用户认证系统，支持邮箱登录和 OAuth"
# 输出：.ai/specs/ideation.md

# 2. 设计阶段
/sy-design .ai/specs/ideation.md
# 输出：.ai/specs/design.md

/sy-code-insight src/
# 输出：.ai/insights/code-analysis.md

# 3. 计划阶段
/sy-writing-plans .ai/specs/design.md
# 输出：.ai/plans/execution-plan.yaml

# 4. 开发阶段
/sy-executing-plans task-001  # 创建 User 模型
/sy-executing-plans task-002  # 实现密码加密
/sy-executing-plans task-003  # 实现注册 API
/sy-executing-plans task-004  # 实现登录 API
/sy-executing-plans task-005  # 实现 OAuth

# 5. 调试（如果需要）
/sy-debug "登录 API 返回 500 错误"

# 6. 验证阶段
/sy-verification-before-completion
# 输出：验证报告

# 7. 审查阶段
/sy-requesting-code-review @reviewer
/sy-receiving-code-review review-comments.md

# 8. 部署阶段
/sy-deployment-automation production

# 9. 维护阶段
/sy-monitoring-alerts
```

---

## 【第十章：Windows 平台优化】

### 10.1 路径处理

**Windows 路径规范**：
```javascript
// 使用反斜杠（Windows 原生）
const filePath = 'D:\\Projects\\seeyue-workflows\\.ai\\specs\\design.md';

// 或使用 path.join（跨平台兼容）
const path = require('path');
const filePath = path.join(process.cwd(), '.ai', 'specs', 'design.md');
```

### 10.2 PowerShell 集成

**执行 PowerShell 脚本**：
```javascript
const { execSync } = require('child_process');

// 执行 PowerShell 命令
execSync('powershell.exe -ExecutionPolicy Bypass -Command "Get-Process"');

// 执行 PowerShell 脚本
execSync('powershell.exe -ExecutionPolicy Bypass -File scripts/deploy.ps1');
```

### 10.3 Git 集成

**Windows Git 配置**：
```bash
# 配置行尾符
git config core.autocrlf true

# 配置长路径支持
git config core.longpaths true

# 配置文件权限
git config core.filemode false
```

---

**文档版本**：v1.0.0
**最后更新**：2026-03-12
**作者**：seeyue-workflows 架构团队

---

**END OF DOCUMENT**
