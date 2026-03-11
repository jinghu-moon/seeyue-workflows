# seeyue-workflows

一个独立维护的 agent workflow 仓库，用来承载 `skills + hooks + tests + docs` 四层资产。

它不承载业务代码，而是承载可复用、可验证、可分发的工作流本体。

## 仓库定位

- `skills`：定义工作流入口、阶段协议、产物要求和路由规则
- `hooks`：提供运行时硬约束，防止阶段越界、危险写入和不完整收口
- `tests`：对 hooks 行为和 skill 触发做回归验证
- `docs`：维护 schema、采用指南、同步说明和发布清单

## 当前主约定

- Canonical session：`.ai/workflow/session.yaml`
- Legacy fallback：`.ai/workflow/session.md`
- Index baseline：`.ai/index.json`
- Hook config：`.claude/settings.json` + `.claude/sy-hooks.policy.json`

## 建议使用方式

### 作为工作流母仓库

1. 在本仓库开发和验证 workflow 本体
2. 在业务仓库中从本仓库同步 workflow 资产
3. 改动优先进入本仓库，业务仓库只接收已验证版本

### 在业务仓库中同步

- `python "seeyue-workflows/scripts/sync-workflow-assets.py" --target-root "."`
- `python "seeyue-workflows/scripts/sync-workflow-assets.py" --target-root "." --check`

## 常用命令

- `npm run test:hooks:smoke`
- `npm run test:skills:core`
- `npm run test:skills:constraints`
- `npm run test:skills:smoke:auto`
- `npm run test:skills:smoke:runner`
- `npm run test:all`

## 许可证

- 本项目采用 AGPL-3.0-or-later 许可证，详见 LICENSE。
- 额外说明见 NOTICE。
