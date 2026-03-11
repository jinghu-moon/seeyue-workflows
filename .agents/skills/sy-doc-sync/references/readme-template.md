# README Template: AI Context Injection

README.AI.md is the primary entry point for AI agents to understand a module.

## Section Order

Ordered by AI reasoning priority:

| # | Section | AI Purpose |
|---|---------|------------|
| 1 | Metadata | Identify type and responsibility |
| 2 | Context | Why it exists, what problem it solves |
| 3 | Architecture | Internal file structure and data flow |
| 4 | Interface Schema | Contract: what can and cannot be changed |
| 5 | Constraints | Avoid breaking changes |
| 6 | Logic & Behavior | State transitions and decision rules |
| 7 | Dependencies | Change impact scope |
| 8 | Patterns | Correct usage and composition |

## Template

### Metadata

```markdown
# [Module Name]

> **Type**: `Component` | `Function` | `Module` | `Class`
> **Status**: `Stable` | `Experimental` | `Deprecated`
> **Responsibility**: [Single sentence: core responsibility]
```

### Context

```markdown
## Context

- **Problem**: Users need [specific requirement]
- **Role**: Responsible for [specific duty] in [system/flow]
- **Split status**: ✅ Focused / ⚠️ Consider splitting
- **Collaborators**: `ModuleA` (provides data), `ModuleB` (consumes events)
```

### Architecture (Medium/Complex only)

```markdown
## Architecture

ModuleName/
├── main.rs              # Main entry
├── sub/
│   ├── handler.rs       # Core logic
│   └── types.rs         # Type definitions

**Data flow** (Mermaid):
graph TD
    A[Entry] --> B[Handler]
    B --> C[State]
```

### Interface Schema

```markdown
## Interface Schema

### Parameters / Inputs

interface Params {
  id: string               // required
  mode: 'edit' | 'view'   // required, default: 'view'
}

| Param | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | `string` | ✓ | — | Unique identifier |
| `mode` | `'edit' \| 'view'` | ✓ | `'view'` | Operation mode |

### Enum Values

| Value | Behavior |
|-------|----------|
| `'edit'` | Enable modifications |
| `'view'` | Read-only |
```

### Constraints

```markdown
## Constraints

**Invariants:**
- MUST NOT mutate inputs directly
- MUST NOT call blocking ops in async contexts

**Error Handling:**

| Scenario | Condition | Behavior |
|----------|-----------|----------|
| Invalid input | `id` is null | Throw `ValidationError` |
| Timeout | > 5s | Retry 3×, then error |
```

### Logic & Behavior

```markdown
## Logic & Behavior

### Decision Rules

- `IF mode == 'view' THEN disable modifications`
- `enableActions = mode == 'edit' AND hasPermission`

### State Strategy

- **Source**: `Ref<T> | Arc<Mutex<T>>` (minimal state)
- **Derived**: `computed(() => state.value?.name)`
- **Effects**: `watch(id, fetchData)` (side effects)
```

### Dependencies

```markdown
## Dependencies

| Type | Package | Purpose |
|------|---------|---------|
| Internal | `crate::api` | Data fetch |
| External | `tokio@^1.0` | Async runtime |
| Peer | `GlobalConfig` | MUST be provided |
```

### Patterns

```markdown
## Patterns

### Basic Usage

fn main() { handler(id: "123", mode: "edit"); }

### ❌ Anti-Patterns

handler(mode: "edit"); // Missing required id
// → Error: id required

mutate_input(&mut param); // VIOLATES invariant
```

### Source Manifest (MUST for incremental sync)

````markdown
## Source Manifest

```yaml
source_manifest:
  schema: 1
  generated_at: 2026-03-04T10:00:00Z
  base_ref: working-tree
  files:
    - path: src/module/main.rs
      fingerprint: stat:2048-1762215600
    - path: src/module/types.rs
      fingerprint: stat:1024-1762215620
```
````
