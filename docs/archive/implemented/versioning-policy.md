# Versioning Policy

## 目标

这份文档定义 `seeyue-workflows` 如何做版本化、如何声明 adapter version，以及什么情况下属于 breaking change。

## Version Model

- `workflow version`：仓库整体发布版本，以 `package.json.version` 为准
- `adapter version`：各 engine adapter 的输出契约版本，由 `sync-manifest.json` 中的 `release.adapter_versions` 声明
- `schema_version`：`sync-manifest.json` 自身的结构版本，不等同于 workflow release 版本

当前规则：

1. `workflow version` 使用 semver
2. `CHANGELOG.md` 最新一条发布记录必须和 `workflow version` 一致
3. `sync-manifest.json.release.workflow_version` 必须和 `workflow version` 一致
4. `adapter version` 可以独立演进，但每次发布都必须显式列出

## Semver Rules

- `MAJOR`：breaking change
- `MINOR`：新增兼容能力或新增发布资产
- `PATCH`：兼容性修复、文档修复、回归修复

以下情况通常应提升 `MAJOR`：

- 修改 `workflow/*.yaml` 的不兼容字段语义
- 修改 hooks / adapter 的核心输入输出约定
- 修改 sync boundary，导致消费仓库必须人工迁移

## Adapter Version Rules

每次发布都要在 `sync-manifest.json` 中记录：

- `claude_code` adapter version
- `codex` adapter version
- `gemini_cli` adapter version

如果 workflow version 升级但 adapter contract 没变，adapter version 可以保持不变。

## Breaking Change Contract

如果 `sync-manifest.json.release.breaking_change=true`，则必须同时满足：

1. `CHANGELOG.md` 记录破坏性变化
2. `sync-manifest.json.release.upgrade_notes` 非空
3. `minimum_sync_version` 已更新
4. 发布前完成一次消费仓库升级演练

禁止只改 `breaking_change` 标记而不写升级说明。

## Sync Contract

`sync-manifest.json` 是同步边界声明，不是聊天约定。

每次发布必须确认：

- 需要分发的 docs 已纳入 manifest
- 需要分发的 skills / hooks / tests 已纳入 manifest
- 不需要分发的业务代码没有进入 manifest

## Release Order

建议顺序：

1. 更新 `workflow version`
2. 更新 `CHANGELOG.md`
3. 更新 `sync-manifest.json.release`
4. 运行 `node tests/e2e/run-release-fixtures.cjs`
5. 运行 `node tests/e2e/run-engine-conformance.cjs --all`
6. 运行 `node tests/e2e/run-doc-link-check.cjs`

## Source Files

- [`package.json`](../../../package.json)
- [`CHANGELOG.md`](../../../CHANGELOG.md)
- [`sync-manifest.json`](../../../sync-manifest.json)
- [`docs/archive/implemented/release-checklist.md`](./release-checklist.md)
- [`docs/archive/implemented/source-of-truth.md`](./source-of-truth.md)
