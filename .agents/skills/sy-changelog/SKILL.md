---
name: sy-changelog
description: Use when recording, merging, viewing, or clearing structured feature-level change logs for downstream commit/release message generation.
argument-hint: "[action, scope]"
disable-model-invocation: false
---

# Changelog

Record development changes at feature granularity. Output to `change-log.txt` (project root).

## Operations

### 1. Record (default)

Triggers: `记录变更`, `记一下`, `log`, `changelog`

Steps:
1. Analyze current session for completed features/changes
2. If context insufficient, run `git diff --name-status`
3. Generate one entry per feature with affected file list
4. Append to `change-log.txt`

Pre-write checks:
- File missing → create with header `# 澄镜变更日志\n`
- Date section missing → insert `## YYYY-MM-DD`
- Type section missing → insert `### <type>`

### 2. Merge

Triggers: `合并日志`, `合并变更`, `merge log`

Strategies:
- **Default**: Merge entries with same date + type + scope. Keep latest description, deduplicate file list.
- **Cross-date**: On `合并 <scope>`, merge all entries of that scope across dates into the latest date section. For long-running feature work.

### 3. View

Triggers: `查看变更`, `看日志`, `show log`

Read and display `change-log.txt`.

### 4. Clear

Triggers: `清空日志`, `clear log`

Reset `change-log.txt` to header only. Requires user confirmation.

## Format

```
# 澄镜变更日志

## 2026-02-11

### feat
- **SearchBar**: 新增搜索建议功能
  - `src/components/SearchBar/index.vue`
  - `src/components/SearchBar/types.ts`

### fix
- **NotePad**: 修复保存时内容丢失问题
  - `src/components/NotePad/index.vue`
```

## Rules

- Granularity: one feature = one entry, never split by file
- Scope: follow sy-git-commit skill's Scope Mapping
- Type: `feat` / `fix` / `refactor` / `style` / `docs` / `chore` / `perf` / `test`
- Description: Chinese, ≤ 30 chars, verb-first
- File list: relative paths only, no status prefix
- Repeated changes to same feature → append new entry (merge is manual)
- Ignored files:
  - Auto-generated: `auto-imports.d.ts`, `components.d.ts`
  - Directories: `node_modules/`, `dist/`, `build/`
  - Lock files: `package-lock.json`, `pnpm-lock.yaml`
  - OS files: `.DS_Store`, `Thumbs.db`

## Git-Commit Integration

When sy-git-commit skill triggers and `change-log.txt` has entries:

1. Prompt user: "Changelog detected. Use it for commit message? (yes / no / skip)"
   - **yes**: read log, generate commit message per mode below
   - **no**: ignore log, proceed with normal sy-git-commit flow
   - **skip**: skip logging entirely (for trivial changes)

2. Output modes:
   - **brief** (default): `type(scope): description` — pick the most significant entry
   - **verbose** (trigger: `详细提交`): append file change list as commit body (max 15 files; if exceeded, top 15 + `... and N more`). If entries span multiple dates, append summary: `"(合并了 N 天的变更)"`

3. Multi type/scope resolution:
   - Single scope → use directly
   - Multiple scopes → sort by file count desc, recommend top one, fallback `app` on Enter
   - Description from most significant change

4. After successful commit, prompt to clear log
