# Operation: Update

Sync existing README.AI.md with current code reality.

## Precondition

- README.AI.md MUST exist at target path
- IF absent → abort, suggest `编写文档`

## Steps

1. Read existing README.AI.md → parse current structure
2. Load latest insight cache from `.ai/insights/<path-key>.json` (if available)
3. Validate README `Source Manifest` against current files (prefer script):
   - run `pwsh -File ".agents/skills/sy-doc-sync/scripts/check-manifest-file-state.ps1" -ReadmePath "<target>/README.AI.md" -ProjectRoot "<repo-root>" -AsJson`
   - script basis: file `size + mtime` (`stat:<size>-<mtime_epoch>`)
   - script also uses `git diff --name-status -M` for rename hints when repo is available
4. Determine update mode (`SKIP | INCREMENTAL | FULL | REINDEX`)
5. Read only delta-relevant code first; expand scope only if evidence insufficient
6. Compare: code reality vs README documentation
7. Identify outdated sections (changed interfaces, new constraints, modified logic)
8. Update ONLY outdated sections, preserve accurate content
9. Regenerate `Source Manifest` baseline after doc update:
   - run `pwsh -File ".agents/skills/sy-doc-sync/scripts/generate-source-manifest.ps1" -TargetPath "<target>" -ProjectRoot "<repo-root>"`
   - or auto-write back with `-ReadmePath "<target>/README.AI.md" -WriteToReadme`
10. Self-check validation metrics
11. Apply writing rules per references/writing-style.md
12. Save updated README.AI.md

## Update Rules

- Preserve structure and formatting style
- MUST NOT regenerate unchanged sections (git-friendly diffs)
- New interface → update Interface Schema section
- Logic changes → update Logic & Behavior section
- Deprecated features → mark as `deprecated` in Patterns
- Complexity tier increase → add missing sections
- IF delta is empty → keep file unchanged and output "No doc changes required"
- README MUST include machine-readable `Source Manifest` block for next-run verification
- `Source Manifest` fingerprints MUST use `stat:<size>-<mtime_epoch>` generated from current files

### Source Manifest Minimum Fields

```yaml
source_manifest:
  schema: 1
  generated_at: ISO-8601
  base_ref: <git-commit-or-working-tree>
  files:
    - path: src/module/file.ts
      fingerprint: stat:<size>-<mtime_epoch>
```

## Output

```
Updated <path>/README.AI.md
Changes:
  - Interface: Added 2 new params
  - Constraints: Updated validation rule
  - Patterns: Marked Pattern 3 as deprecated
Delta Basis:
  - changed files: <n>
  - deleted files: <n>
  - renamed files: <n>
  - cache used: <yes/no>
  - update mode: SKIP | INCREMENTAL | FULL | REINDEX
```
