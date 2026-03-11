# Operation: Init

Project-level architecture audit. Read-only, evidence-based.

## Precondition

- New session or user explicitly requests project-level analysis
- Identify available read-only tools before proceeding

## Internal Audit Model

- **Student**: Analyze facts, flag unknowns as Blind Spots.
- **Teacher**: Reject unsupported claims, request tool verification.

> Execute internally. Do NOT expose reasoning or drafts.
> Max 2 iterations (default). In --deep mode: Loop until Score >= 95% or no Blind Spots.
> IF context budget constrained → single-pass with Blind Spot marking. Skip iteration.

## Terminology

| Term | Definition |
|------|-----------|
| Coding Standards | Project's conventions, policy files, style guides (`CLAUDE.md`, `AGENTS.md`, `.codex/`, linter configs) |
| Blind Spot | Claim or area where direct evidence is missing |
| Evidence | Exact quotes, code snippets, or config values from project files |

## Rules (RFC 2119)

- MUST extract exact quotes before forming conclusions
- Claims without evidence MUST be marked as Blind Spot
- Missing source code → output "No evidence provided", MUST NOT guess
- Uncertain API → SHOULD Web Search official docs
- MUST NOT infer function from file/dir names; MUST read code
- MUST self-check against universal validation metrics (SKILL.md)

## Analysis Layers (Sequential)

1. **Layer 1 - Coding Standards**: Policy files, conventions, linter configs, style guides
2. **Layer 2 - Build & Execution Flow**: Scripts, manifests, entry points
3. **Layer 3 - Backend Interfaces**: IPC / FFI / system calls (evidence-based only)
4. **Layer 4 - Frontend / Caller Interaction**: UI model, API consumers (evidence-based only)

## Tool Usage (Abstract Layer)

All tool usage MUST be read-only. Agent maps to available tools:

| Operation | Claude Code | Codex | Fallback |
|-----------|------------|-------|----------|
| File inspection | `Read`, `Glob` | `cat`, `ls` | Manual paste |
| Pattern search | `Grep`, `Task(Explore)` | `grep`, `rg` | Ctrl+F |
| Web lookup | `WebSearch`, `WebFetch` | `web_search` | Mark as Blind Spot |
| Symbol analysis | `LSP`, Serena tools | `grep -n` | Mark as Blind Spot |

> Unavailable tools → state explicitly. Do NOT fail silently.

## Steps

1. Capability Check: identify available read-only tools
2. Discover project root: locate config files, policy files, manifests
3. Detect project type/language → Load lang-extensions/xx.md
4. Layer 1: extract Coding Standards and constraints
5. Layer 2: trace build/execution flow
6. Layer 3: identify backend interfaces (if applicable)
7. Layer 4: identify frontend/caller interaction (if applicable)
8. Self-check validation metrics
9. Internal Teacher-Student audit (iterations per mode)
10. Generate index baseline:
    - `pwsh -File ".agents/skills/sy-code-insight/scripts/update-index.ps1" -ProjectRoot "." -OutputPath ".ai/index.json"`
    - `pwsh -File ".agents/skills/sy-code-insight/scripts/validate-index.ps1" -IndexPath ".ai/index.json"`
11. Output diagnostic report + Persist to .ai/init-report.md (Markdown, start with Generated: YYYY-MM-DDTHH:MM:SSZ)

## Progress Control

### Step 1: Scope Assessment
Before reading any file, agent MUST:
1. List all files in scope (project root)
2. Exclude: dependencies, generated code, lock files, static assets, test fixtures
3. Output: "Scope: N files require understanding"

### Step 2: Smart Filtering

| Need Understanding | Skip |
|-------------------|------|
| Entry points (main.rs, App.vue) | node_modules/, target/, dist/ |
| Public interfaces (mod.rs, index.ts) | Test fixtures, mock data |
| Config files (Cargo.toml, vite.config) | Generated code (*.generated.*) |
| Core business logic | Vendored third-party code |
| Type definitions (types.ts, model.rs) | Static assets (images, fonts) |
| Build scripts | Lock files (Cargo.lock, package-lock) |

### Step 3: Priority Ordering
1. Entry points and manifests (MUST read first)
2. Public interface files (mod.rs, index.ts, types)
3. Core logic files
4. Supporting utilities
5. Config and scripts

### Step 4: Progressive Reading
- Track: understood_count / total_count
- At each file: decide "signatures only" or "full body"
- IF file > 500 lines → signatures only + mark body as Blind Spot
- IF understood_count / total_count >= 90% → STOP
- Remaining files → mark as Blind Spot in report
- Exit: "Understanding: X/Y files (Z%). Blind Spots: N files."

## Output Schema

### 1. Metadata

```
- Type: [Desktop App | Web App | CLI Tool | Browser Extension | Library]
- Language: [Primary language(s)]
- Core_Loop: [One-sentence primary control/data flow]
- Coding Standards Found: [List of policy/style files, or "None"]
```

### 2. Imposed Constraints (Verified)

List ONLY constraints with direct evidence.

```
- [CONSTRAINT]: Description
  Evidence: [Exact quote or file:line reference]
```

### 3. IPC & System Interface

> If no IPC/FFI/system calls detected, output "Not applicable" and skip table.

| Trigger | Command | Backend Impl | Permissions | Evidence Quote |
|---------|---------|--------------|-------------|----------------|
| [Event] | [Name]  | [Source]     | [Perms]     | [Exact quote]  |

### 4. Diagnostics & Final Verdict

**Understanding_Score**: [0-100%]

```
90-100%  All four Layers have evidence coverage
70-89%   Layer 1-2 complete, Layer 3-4 have Blind Spots
50-69%   Layer 1 complete, Layer 2+ have Blind Spots
< 50%    Layer 1 has unverified constraints
```

**Ready_Nodes**: [Safe-to-modify files, or "None identified"]

**Blind_Spots**:
```
- [Layer]: [What is unknown]
```

**Teacher_Flagged_Risks** (MUST list at least one):
```
1. [Risk with evidence reference]
2. [Additional risk/question]
```

## Auto-Chaining

After completion, suggest `理解 <path>` for Ready_Nodes. In --deep: Auto-execute `理解` on Blind Spots.
