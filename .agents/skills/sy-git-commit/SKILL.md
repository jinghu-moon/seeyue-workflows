---
name: sy-git-commit
description: Use when user explicitly requests commit or release message generation after code changes are complete and reviewed.
allowed-tools:
  - Read
  - Bash(git:*)
disable-model-invocation: true
metadata:
  version: "2.0.0"
  updated: "2026-01-29"
argument-hint: "[context]"
---

# Git Commit Message Generator

Generate commit messages for ChengJing project, formatted for direct use with release script.

## Output Format

Always output in this exact format:

```bash
npm run release "commit message here"
```

---

## Message Generation Rules

### 1. Source Analysis (Priority Order)

1. **Conversation Context**: What was accomplished in this session?
2. **File Changes**: Check recent edits or `git status --short`
3. **Scope Detection**: Identify main component/module affected

### 2. Conventional Commits Format

```
<type>(<scope>): <subject in Chinese>
```

| Type | Usage |
|------|-------|
| `feat` | New feature |
| `fix` | Bug fix |
| `refactor` | Code refactoring |
| `style` | Styling/formatting |
| `docs` | Documentation |
| `chore` | Tooling/deps/config |
| `perf` | Performance |
| `test` | Testing |

### 3. Scope Mapping

| Folder Pattern | Scope |
|----------------|-------|
| `DailyPoem/` | DailyPoem |
| `BookmarkPanel/` | Bookmark |
| `ShortcutGrid/` | Shortcut |
| `NotePad/` | NotePad |
| `SettingsPanel/` | Settings |
| `.agent/skills/` | skills |
| `vite.config.ts` | build |
| `src/styles/` | styles |
| Multiple components | app |

### 4. Subject Guidelines

- **Language**: Chinese (中文) - Required
- **Length**: ≤ 50 characters
- **Tense**: Start with verb (新增, 修复, 优化, 重构, 更新)
- **Punctuation**: No period at end

---

## Examples

### Single Feature
```bash
npm run release "feat(DailyPoem): 新增诗词分享卡片组件"
```

### Bug Fix
```bash
npm run release "fix(Bookmark): 修复拖拽排序失效问题"
```

### Refactoring
```bash
npm run release "refactor(SearchInput): 使用 defineModel 重构双向绑定"
```

### Multiple Changes
```bash
npm run release "feat(app): 新增 VueUse 支持并优化 Skills 体系"
```

### Config/Tooling
```bash
npm run release "chore(skills): 完善设计系统与 MV3 最佳实践文档"
```

### Styles
```bash
npm run release "style(styles): 新增语义化间距与无障碍令牌"
```

---

## Workflow

When user says "提交", "commit", or "release":

1. Check if `change-log.txt` exists and has entries
   - **Has entries**: Default behavior is **use sy-changelog**. Show one-line confirmation:
     `"基于变更日志生成 commit message (Enter=确认 / n=跳过)"`
   - **No file / empty**: Proceed to step 2 normally

2. Analyze source (priority: changelog entries > conversation context > git diff)
3. Scope resolution:
   - Single scope → use directly
   - Multiple scopes → sort by file count desc, recommend top one:
     `"检测到多个 scope: 1) SearchBar (5 files) ★ 2) NotePad (2 files) 3) Settings (1 file). 回车使用推荐 / 输入序号选择:"`
4. Generate message in **Chinese**
5. Output: `npm run release "message"`
6. On success, prompt user to clear `change-log.txt` if it was used

### Output Modes

- **brief** (default): `type(scope): description` — single most significant change
- **verbose** (trigger: `详细提交`): Same title + file change list as commit body (max 15 files; if exceeded, show top 15 + `... and N more`). If entries span multiple dates, append summary: `"(合并了 N 天的变更)"`
