---
name: sy-doc-sync
description: Use when a module-level `README.AI.md` must be created or incrementally synchronized with current code and interfaces.
argument-hint: [action] [path]
disable-model-invocation: false
---

# Doc Sync (README.AI)

Write-side documentation layer for module AI docs.

| Phase | Scope | Operation | Trigger |
|-------|-------|-----------|---------|
| 1 | Module | Write | `编写文档 <path>` |
| 2 | Module | Update | `更新文档 <path>` |

## Trigger

| Trigger | Operation | Side Effects |
|---------|-----------|--------------|
| `编写文档 <path>` | Create README.AI.md | Creates file |
| `更新文档 <path>` | Sync README.AI.md with code | Modifies file |

**Auto-routing**: `IF bare 更新 <path> AND README.AI.md exists → 更新文档` | `IF absent → 编写文档`

## Incremental Update Policy (Default)

- `更新文档` MUST use incremental update by default
- Incremental basis:
  - changed files / changed interfaces
  - cached insights from `.ai/insights/*`
  - existing README.AI.md structure
- MUST NOT regenerate whole README when partial section update is sufficient

## Operations

- **Write**: [operations/write-readme.md](operations/write-readme.md)
- **Update**: [operations/update-readme.md](operations/update-readme.md)

## References

- [references/readme-template.md](references/readme-template.md): README.AI.md structure template
- [references/writing-style.md](references/writing-style.md): AI-first writing principles (7 principles)
- references/lang-extensions/: Language-specific rules
  - [vue.md](references/lang-extensions/vue.md): Props/Emits/Reactivity
  - [rust.md](references/lang-extensions/rust.md): Features/Unsafe/FFI
  - [ts.md](references/lang-extensions/ts.md): Interfaces/Generics
- scripts:
  - [scripts/check-manifest-file-state.ps1](scripts/check-manifest-file-state.ps1): Validate README Source Manifest by `size + mtime` and return update mode
  - [scripts/generate-source-manifest.ps1](scripts/generate-source-manifest.ps1): Generate `Source Manifest` from files and optionally write back to README.AI.md

## Universal Validation Metrics (MUST Self-Check)

**Hard Metrics** (Mechanically verifiable):

| Metric | Verification |
|--------|-------------|
| Entry Point | file:line exists and matches code |
| Interfaces Complete | Output count == actual pub/exports |
| Dependencies Complete | Output == actual use/import |
| Evidence Coverage | Every claim has file:line or Blind Spot |
| No Orphan Claims | Zero unsupported statements |

## Complexity Tiers

| Tier | Condition | Sections |
|------|-----------|----------|
| Simple | < 100 lines, single file | Metadata + Context + Interface + Constraints + Patterns |
| Medium | 100-500 lines, 2-5 files | All 8 sections + Architecture diagram |
| Complex | 500+ lines, many files | All 8 sections + Mermaid diagrams + multiple Patterns |
