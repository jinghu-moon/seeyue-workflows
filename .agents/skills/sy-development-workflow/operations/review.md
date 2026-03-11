# Operation: Review

Post-execution self-check and documentation sync. Runs after all nodes complete.

## Precondition

- All Micro-Nodes in execute phase completed successfully

## Steps

1. Collect all modified files from execute phase
2. Run full verification (compile + test suite + lint + build + coverage gate when applicable)
3. Load reviewer prompt and stack persona:
   - `.agents/skills/sy-code-insight/references/prompts/reviewer.prompt.md`
   - stack source: `.ai/analysis/ai.report.json.project.language_stack` (fallback to init/deps)
   - prompt language MUST be English, audience MUST be agent
4. Stage-A Spec Compliance Review:
   - verify every node outcome vs source-of-truth and approved plan
   - reject over-build / under-build
5. Stage-B Code Quality Review:
   - severity-ordered findings (Critical > High > Medium > Low)
   - evidence per finding (`file:line` / command output)
6. Self-check: compare changes against original plan
7. Constraint audit: apply `sy-workflow-constraints` phase checklist
8. Phase compliance audit: ensure no future-phase scope creep
9. Refresh final dual report outputs:
   - run `pwsh -File ".agents/skills/sy-code-insight/scripts/generate-ai-report.ps1" -Task "<task-description>" -UpdateMode "<mode>" -ChangedFiles <n> -DeletedFiles <n> -RenamedFiles <n> -PhaseId "<Px>" -NodeId "<last-node>" -Compile <pass|fail|skip> -Test <pass|fail|skip> -Lint <pass|fail|skip> -Build <pass|fail|skip>`
   - validate JSON report:
     - `pwsh -File ".agents/skills/sy-code-insight/scripts/validate-ai-report.ps1" -ReportPath ".ai/analysis/ai.report.json" -SchemaPath ".agents/skills/sy-code-insight/references/ai-report.schema.json"`
10. Call `sy-doc-sync: 更新文档 <path>` for each modified module (incremental by delta)
11. If review feedback is provided:
   - run `处理评审反馈 <feedback-source>` and apply verify-first loop
12. Optional handoff (only if user requests):
   - call `sy-changelog` to record release notes
   - call `sy-git-commit` to prepare commit message
13. Output summary and ask user whether to proceed to next phase

## Self-Check

| Check | Pass Condition |
|-------|----------------|
| Plan coverage | Every planned node was executed |
| No scope creep | No unplanned files modified |
| Tests pass | Full test suite green |
| Coverage gate | Actual coverage meets required minimum or explicitly N/A |
| Constraint compliance | `sy-workflow-constraints` phase checklist passed |
| Phase gate | No future-phase implementation introduced |
| API fidelity | Uncertain APIs were verified against official docs |
| Reviewer persona | Reviewer prompt loaded with stack-matched persona |
| Evidence before claim | All success claims backed by fresh command evidence |
| Docs synced | README.AI.md updated for changed modules |
| Report synced | `.ai/analysis/ai.report.md` + `.ai/analysis/ai.report.json` refreshed |
| Report schema valid | `validate-ai-report.ps1` passed |

## Output Format

```
## Review: <task-description>

Phase: <P1/P2/...>
Nodes: 3/3 completed
Files modified: 3
Validation: compile ✓ | tests ✓ | lint ✓ | build ✓
Coverage:  <actual>% / <required>% | N/A

Changes:
  - src/types.rs: Added DryRunConfig interface
  - src/redirect.rs: Implemented dry_run()
  - tests/redirect_test.rs: 4 new test cases

Docs updated:
  - src/redirect/README.AI.md (Interface + Patterns sections)
Reports updated:
  - .ai/analysis/ai.report.md
  - .ai/analysis/ai.report.json

No scope creep detected. Phase constraints satisfied.

✅ Phase <Px> complete. Please review.
Should I proceed to Phase <P{x+1}>?
```
