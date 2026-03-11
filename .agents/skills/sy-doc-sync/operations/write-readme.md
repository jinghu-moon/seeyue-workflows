# Operation: Write

Analyze code → create README.AI.md for modules without documentation.

## Precondition

- Target path MUST NOT contain README.AI.md
- IF exists → abort, suggest `更新文档`

## Steps

1. Load lang-extensions/xx.md based on type
2. Read all code files at target path
3. Analyze: interfaces, types, dependencies, state logic, constraints
4. Self-check validation metrics
5. Determine complexity tier (SKILL.md)
6. Generate README.AI.md per references/readme-template.md
7. Generate Source Manifest baseline:
   - run `pwsh -File ".agents/skills/sy-doc-sync/scripts/generate-source-manifest.ps1" -TargetPath "<path>" -ProjectRoot "<repo-root>"`
   - append generated `## Source Manifest` section with `stat:<size>-<mtime_epoch>` fingerprints
8. Apply writing rules per references/writing-style.md
9. Save to `<path>/README.AI.md`

## Output

```
Created <path>/README.AI.md
Complexity: [Simple|Medium|Complex]
Sections: Metadata / Context / Architecture / Interface / Constraints / Logic / Deps / Patterns
```
