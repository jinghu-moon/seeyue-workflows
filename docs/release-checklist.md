# 发布检查清单

## 目标

这份清单用于发布 `seeyue-workflows` 时，确保版本、同步清单、文档和验证证据保持一致。

发布前先阅读：

- [版本化策略](./versioning-policy.md)
- [事实源说明](./source-of-truth.md)
- [运行手册](./operations-runbook.md)
- [V4 架构方案](./architecture-v4.md)
- [变更日志](../CHANGELOG.md)

## 发布前

### 机器规范与运行态

- [ ] `node scripts/runtime/validate-specs.cjs --all`
- [ ] `npm run test:runtime:p2`
- [ ] `npm run test:runtime:context`
- [ ] `npm run test:runtime:recovery`

### hooks 与 adapter

- [ ] `npm run test:hooks:smoke`
- [ ] `node tests/adapters/run-adapter-snapshots.cjs --engine claude_code`
- [ ] `node tests/adapters/run-adapter-snapshots.cjs --engine codex`
- [ ] `node tests/adapters/run-adapter-snapshots.cjs --engine gemini_cli`
- [ ] `npm run test:e2e:engine-conformance`

## 同步前

- [ ] `node tests/e2e/run-doc-link-check.cjs`
- [ ] `npm run test:e2e:release`
- [ ] [`../sync-manifest.json`](../sync-manifest.json) 已包含 skills、hooks、tests、docs 的发布边界
- [ ] [`./versioning-policy.md`](./versioning-policy.md) 与 [`../CHANGELOG.md`](../CHANGELOG.md) 的版本语义一致
- [ ] [`./adoption-guide.md`](./adoption-guide.md)、[`./source-of-truth.md`](./source-of-truth.md)、[`./operations-runbook.md`](./operations-runbook.md) 已同步更新

## 发布后

- [ ] `package.json.version`、`CHANGELOG.md`、`sync-manifest.json.release.workflow_version` 三者一致
- [ ] 三个 engine vendor 输出与 [`../CLAUDE.md`](../CLAUDE.md)、[`../AGENTS.md`](../AGENTS.md)、[`../GEMINI.md`](../GEMINI.md) 对齐
- [ ] 同步脚本 `--check` 通过，且无额外漂移
- [ ] 发布记录已回填到运行手册或发布说明

## 回滚

如果发布后发现问题，按以下顺序回滚：

1. 回退 `workflow/*.yaml` 或相关 runtime 代码
2. 重新生成并覆盖 vendor 文档
3. 重新校正 [`../sync-manifest.json`](../sync-manifest.json) 的边界声明
4. 在 [`../CHANGELOG.md`](../CHANGELOG.md) 追加回滚说明
