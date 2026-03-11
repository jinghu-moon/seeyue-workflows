# 仓库结构说明

## 分层

### 1. Skills

负责定义 workflow 入口、阶段协议、产物结构和路由规则。

### 2. Hooks

负责执行运行时硬约束：
- session 状态合法性
- 调试 / TDD / 阶段边界约束
- stop gate
- 审计与 index 失效

### 3. Tests

- `tests/hooks`：hook 行为回归
- `tests/skill-triggering`：skill trigger 回归
- 建议同时保留 `local / auto / runner` 三档

### 4. Docs

- session schema
- adoption guide
- release checklist
- migration / source-of-truth 说明
