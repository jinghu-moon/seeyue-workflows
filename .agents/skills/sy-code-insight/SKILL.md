---
name: sy-code-insight
description: Use when project/task/module understanding is required before planning or implementation, and analysis artifacts must be persisted under `.ai/` for session continuity.
argument-hint: "[operation, target, options]"
disable-model-invocation: false
---

# Code Insight (Language Neutral)

Read-only understanding layer for project/task/module context.

| Phase | Scope | Operation | Trigger |
|-------|-------|-----------|---------|
| 1 | Project | Init | `初始化 [--deep]` |
| 2 | Task | Analyze | `分析 <task-description>` |
| 3 | Module | Read | `理解 <path>` |

## Trigger

| Trigger | Operation | Side Effects |
|---------|-----------|--------------|
| `初始化` | Project architecture audit → .ai/init-report.md | Creates file |
| `初始化 --deep` | Deep audit, auto-resolve Blind Spots | Creates file |
| `分析 <task-description>` | Task-driven impact analysis | Read-only |
| `理解 <path>` | Structured JSON output | Read-only |

> Read-only means no source/business file modification.
> This skill MUST persist analysis artifacts under `.ai/` for continuity.

## Operations

- **Init**: [operations/init.md](operations/init.md)
- **Analyze**: [operations/analyze.md](operations/analyze.md)
- **Read**: [operations/read-code.md](operations/read-code.md)

## Persistence Artifacts (MUST)

- `.ai/init-report.md`: project-level baseline
- `.ai/index.json`: project file index + fingerprint baseline + understanding cache anchor
- `.ai/analysis/latest.md`: latest task analysis summary
- `.ai/analysis/<timestamp>-<task>.md`: historical task analyses
- `.ai/analysis/ai.report.md`: human-readable summary report
- `.ai/analysis/ai.report.json`: machine-readable report (automation source of truth, v3 nested tree + project/scm/run/file intelligence)
- `.ai/insights/<path-key>.json`: latest module understanding snapshot

## Incremental Strategy (Default)

- MUST prefer incremental insight updates over full re-read
- Incremental baseline:
  - `.ai/index.json`
  - previous `.ai/analysis/latest.md`
  - existing `.ai/insights/<path-key>.json`
  - working-tree diff (if available)
- IF target not changed and cache is fresh → MAY reuse cache with lightweight validation
- IF cache missing/stale or interfaces changed → MUST perform full read for that target

## References

- references/lang-extensions/: Language-specific rules
  - [vue.md](references/lang-extensions/vue.md): Props/Emits/Reactivity
  - [rust.md](references/lang-extensions/rust.md): Features/Unsafe/FFI
  - [ts.md](references/lang-extensions/ts.md): Interfaces/Generics
- [references/ai-report.schema.json](references/ai-report.schema.json): schema for `.ai/analysis/ai.report.json`
- prompts:
  - [references/prompts/planner.prompt.md](references/prompts/planner.prompt.md): stack-matched planning persona prompt (English, agent-oriented)
  - [references/prompts/author.prompt.md](references/prompts/author.prompt.md): stack-matched execution persona prompt (English, agent-oriented)
  - [references/prompts/reviewer.prompt.md](references/prompts/reviewer.prompt.md): stack-matched review persona prompt (English, agent-oriented)
- scripts:
  - [scripts/update-index.ps1](scripts/update-index.ps1): generate or refresh `.ai/index.json`
  - [scripts/validate-index.ps1](scripts/validate-index.ps1): validate `.ai/index.json`
  - [scripts/generate-ai-report.ps1](scripts/generate-ai-report.ps1): generate `ai.report.md + ai.report.json`
  - [scripts/validate-ai-report.ps1](scripts/validate-ai-report.ps1): validate `ai.report.json` against schema rules (pure PowerShell)

## Language Detection

Agent MUST detect project type at init and load corresponding lang-extension:

| Signal | Language |
|--------|----------|
| Cargo.toml | Rust → lang-extensions/rust.md |
| package.json + .vue files | Vue → lang-extensions/vue.md |
| tsconfig.json | TypeScript → lang-extensions/ts.md |
| Multiple detected | Load all applicable extensions (e.g., Cargo.toml + .vue → rust.md + vue.md) |

The detected stack SHOULD be exposed to workflow prompts via:
- `.ai/analysis/ai.report.json` → `project.language_stack` (preferred)
- fallback: `.ai/init-report.md`

## Universal Validation Metrics (MUST Self-Check)

**Hard Metrics** (Mechanically verifiable):

| Metric | Verification |
|--------|-------------|
| Entry Point | file:line exists and matches code |
| Interfaces Complete | Output count == actual pub/exports |
| Dependencies Complete | Output == actual use/import |
| Evidence Coverage | Every claim has file:line or Blind Spot |
| No Orphan Claims | Zero unsupported statements |

**Soft Metrics** (Inferential):

| Metric | Judgment |
|--------|---------|
| Control Flow | Entry to output paths fully traced |
| Branch Coverage | All #[cfg]/if/routes identified |
| Error Paths | Main exception scenarios recorded |

Self-check failure → Mark Blind Spot and halt.
