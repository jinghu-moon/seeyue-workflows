# 接入指南

## 目标

`seeyue-workflows` 是独立工作流仓库，用来向目标仓库分发 skills、hooks、tests、runtime 规范与适配器输出。
它不承载业务功能代码，重点是把 agent workflow 作为可同步的基础设施提供出去。

接入前先阅读：

- [V4 架构方案](./architecture-v4.md)
- [事实源说明](./source-of-truth.md)
- [版本化策略](./versioning-policy.md)
- [runtime schema](../workflow/runtime.schema.yaml)
- [router spec](../workflow/router.spec.yaml)
- [policy spec](../workflow/policy.spec.yaml)

## 前置条件

- `Node.js >= 22`
- `Python >= 3.11`
- 目标仓库允许同步 `.agents/skills`、`scripts/hooks`、`tests/hooks` 等工作流资产
- 目标环境至少选定一个 engine：`claude_code`、`codex` 或 `gemini_cli`

接入原则：

1. `workflow/*.yaml` 始终是 machine source of truth。
2. `CLAUDE.md`、`AGENTS.md`、`GEMINI.md` 只是 vendor 输出，不反向成为事实源。

## 接入步骤

### 1. 同步 workflow 资产

使用以下脚本同步工作流资产：

- [`scripts/sync-workflow-assets.py`](../scripts/sync-workflow-assets.py)
- [`sync-manifest.json`](../sync-manifest.json)

命令：

```bash
python "<SOURCE_ROOT>/scripts/sync-workflow-assets.py" --target-root "<TARGET_ROOT>"
python "<SOURCE_ROOT>/scripts/sync-workflow-assets.py" --target-root "<TARGET_ROOT>" --check
```

同步边界以 manifest 为准，至少包含：

- `.agents/skills/*`
- `scripts/hooks/*`
- `tests/hooks/*`
- `tests/skill-triggering/*`
- 必需 workflow docs

### 2. 生成 engine vendor 输出

通过 adapter 生成各引擎侧入口文件：

- Claude Code：[`scripts/adapters/claude-code.cjs`](../scripts/adapters/claude-code.cjs)
- Codex：[`scripts/adapters/codex.cjs`](../scripts/adapters/codex.cjs)
- Gemini CLI：[`scripts/adapters/gemini-cli.cjs`](../scripts/adapters/gemini-cli.cjs)

命令：

```bash
node "<SOURCE_ROOT>/scripts/adapters/claude-code.cjs" --root "<SOURCE_ROOT>" --output "<TARGET_ROOT>" --write
node "<SOURCE_ROOT>/scripts/adapters/codex.cjs" --root "<SOURCE_ROOT>" --output "<TARGET_ROOT>" --write
node "<SOURCE_ROOT>/scripts/adapters/gemini-cli.cjs" --root "<SOURCE_ROOT>" --output "<TARGET_ROOT>" --write
```

生成后，核对 [`../CLAUDE.md`](../CLAUDE.md)、[`../AGENTS.md`](../AGENTS.md)、[`../GEMINI.md`](../GEMINI.md) 是否与事实源一致。

### 3. 运行验证

至少执行以下验证：

- [`scripts/runtime/validate-specs.cjs`](../scripts/runtime/validate-specs.cjs)
- [`tests/hooks/sy-hooks-smoke.cjs`](../tests/hooks/sy-hooks-smoke.cjs)
- [`tests/e2e/run-engine-conformance.cjs`](../tests/e2e/run-engine-conformance.cjs)
- [`tests/e2e/run-doc-link-check.cjs`](../tests/e2e/run-doc-link-check.cjs)

命令：

```bash
node "<SOURCE_ROOT>/scripts/runtime/validate-specs.cjs" --root "<SOURCE_ROOT>" --all
node "<SOURCE_ROOT>/tests/hooks/sy-hooks-smoke.cjs"
node "<SOURCE_ROOT>/tests/e2e/run-engine-conformance.cjs" --all
node "<SOURCE_ROOT>/tests/e2e/run-doc-link-check.cjs"
```

## 验证重点

接入验证时，重点确认：

- runtime / router / policy 与目标仓库约束一致
- [运行手册](./operations-runbook.md) 所述恢复路径在目标仓库可执行
- `.ai/workflow/` 状态文件不会与目标仓库现有状态冲突
- 版本边界、兼容约束与 [版本化策略](./versioning-policy.md) 一致

## 常见问题

### 同步检查失败

先运行：

```bash
python "<SOURCE_ROOT>/scripts/sync-workflow-assets.py" --target-root "<TARGET_ROOT>" --check
```

再检查：

1. `sync-manifest.json` 是否缺少必须条目
2. [事实源说明](./source-of-truth.md) 与 [运行手册](./operations-runbook.md) 是否已同步
3. adapter 输出是否覆盖了目标仓库已有 vendor 文件

### 版本不一致

如果 `package.json`、`CHANGELOG.md`、`sync-manifest.json.release.workflow_version` 不一致，先按 [版本化策略](./versioning-policy.md) 校正，再重新执行发布验证。

## 继续阅读

- [运行手册](./operations-runbook.md)
- [发布检查清单](./release-checklist.md)
- [文档索引](./README.md)
