# Operation: Read

Analyze source code → output structured JSON. Read-only for source/business files, with insight persistence to `.ai/insights/`.

## Precondition

- No precondition (works with or without README.AI.md)

## Steps

1. Load previous snapshot `.ai/insights/<path-key>.json` (if exists)
2. Detect whether target changed (git diff/file timestamp/signature comparison)
3. Incremental read policy:
   - Changed target → full read (or signatures first if large)
   - Unchanged target + fresh cache → reuse cache with spot-check
4. Read all required code files at target path (source code is ground truth)
5. IF README.AI.md exists → read and cross-reference
6. IF init was executed → incorporate .ai/init-report.md
7. Load lang-extensions/xx.md based on type
8. Analyze: interfaces, types, dependencies, state logic, constraints
9. Self-check validation metrics
10. Persist insight snapshot to `.ai/insights/<path-key>.json` with updated timestamp/hash
11. Output structured JSON (gradual read: signatures first, bodies on need)

## README.AI.md State Binding (MUST)

When README.AI.md exists, the agent MUST verify whether documented evidence still maps to real files.

1. Parse `Source Manifest` block from README.AI.md (if missing, mark `manifest_state=missing`)
2. Prefer script-based validation:
   - `pwsh -File ".agents/skills/sy-doc-sync/scripts/check-manifest-file-state.ps1" -ReadmePath "<target>/README.AI.md" -ProjectRoot "<repo-root>" -AsJson`
3. For each manifest path, check file existence (`exists/deleted`)
4. Read VCS/worktree status:
   - `git status --porcelain` for modified/deleted/untracked
   - `git diff --name-status -M` for rename hints
5. Compare fingerprint (from manifest) with current content marker:
   - Default marker: `stat:<size>-<mtime_epoch>` (`size + mtime`)
6. Classify doc-file mapping state:
   - `MATCH`: file exists and fingerprint matches
   - `MODIFIED`: file exists but fingerprint differs
   - `DELETED`: file not found
   - `RENAMED`: old path missing + rename hint found
   - `UNKNOWN`: insufficient evidence
7. Derive update mode:
   - all `MATCH` and no related git delta -> `SKIP`
   - partial `MODIFIED` only -> `INCREMENTAL`
   - any `DELETED/RENAMED/manifest_state=missing` -> `FULL`
   - repeated mismatch or index corruption suspicion -> `REINDEX`
8. Output `doc_sync` result and recommendation (`更新文档` when not `SKIP`)

## Output Schema

```json
{
  "path": "src/module",
  "type": "Component | Function | Module | Class",
  "summary": "Single sentence: core responsibility.",
  "context": {
    "problem": "Users need [specific requirement]",
    "role": "Responsible for [specific duty] in [system/flow]",
    "collaborators": ["ModuleA (provides data)", "ModuleB (consumes events)"]
  },
  "architecture": {
    "files": {
      "main.rs": "Main entry",
      "handler.rs": "Core logic"
    },
    "data_flow": "main → handler → state"
  },
  "interfaces": {
    "inputs": {},
    "outputs": [],
    "returns": {}
  },
  "constraints": [
    { "rule": "MUST NOT mutate inputs directly", "evidence": "file:line" },
    { "rule": "MUST handle errors explicitly", "evidence": "file:line" }
  ],
  "state_logic": [
    "IF mode == 'view' THEN read-only",
    "enableActions = mode == 'edit' AND hasPermission"
  ],
  "dependencies": {
    "internal": ["crate::api"],
    "external": ["tokio@^1.0"],
    "peer": ["GlobalConfig MUST be provided"]
  },
  "errors": [
    { "scenario": "id null", "type": "ValidationError", "behavior": "throw" }
  ],
  "patterns": {
    "basic": "handler(id: '123', mode: 'edit')",
    "advanced": "With custom config",
    "anti_patterns": ["handler(mode: 'edit')"]
  },
  "deprecated": [],
  "doc_sync": {
    "manifest_state": "present | missing",
    "update_mode": "SKIP | INCREMENTAL | FULL | REINDEX",
    "mapping": [
      {
        "path": "src/module/file.ts",
        "status": "MATCH | MODIFIED | DELETED | RENAMED | UNKNOWN",
        "evidence": "git-status | hash | mtime"
      }
    ],
    "recommendation": "No doc changes required | 更新文档"
  }
}
```

## Constraints

- MUST NOT modify source/business files (read-only analysis)
- MAY write `.ai/insights/*` as analysis cache
- IF source diverges from README.AI.md → suggest `更新文档`
- README.AI.md evidence MUST be validated against real file state before trust
- Output fields: Trim to relevant (e.g., no interfaces if none)
- After output → print: "Analysis complete. Ready for follow-up queries on [path]."

## Persistence Rules

- MUST create `.ai/insights/` if absent
- `<path-key>` SHOULD be path-safe and deterministic (same target path → same key)
- New run on same path MAY overwrite the same `<path-key>.json` with latest snapshot
- Snapshot SHOULD include `updated_at`, `source_paths`, and a lightweight change marker (e.g., hash/signature)
- Snapshot SHOULD include `doc_sync` summary and latest `update_mode` decision
