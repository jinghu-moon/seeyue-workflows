<!-- SY:GENERATED:BEGIN {"generator":"seeyue-compile-adapter","version":1,"engine":"gemini_cli","pass":"routing","registry_revision":"2026-03-11","spec_hash":"616d60fe5a1f409646f9c0319c70fb30f40b3b7bf6b9006dc97f004dbaae498b"} -->
# GEMINI.md

> Generated artifact for `gemini_cli`. Do not edit manually.
> Vendor files are deployment artifacts. `workflow/*.yaml` remains the machine source of truth.

## Context Hierarchy

- Preserve Gemini CLI hierarchy: global `~/.gemini/GEMINI.md`, workspace `GEMINI.md`, and just-in-time directory `GEMINI.md` files.
- Treat this root `GEMINI.md` as the workspace policy layer, not as a free-form scratchpad.
- Keep directory-local `GEMINI.md` files narrow and component-scoped so JIT loading stays precise.
- Project settings in `.gemini/settings.json` override user and system settings, then extension hooks are merged afterward.

## Language Policy

- Write machine-facing rules, contracts, plans, and skill logic in English.
- Write human-facing approvals, blockers, and status updates in concise zh-CN.
- Keep approval text short, explicit, and action-oriented.
- Human-facing approval requests MUST use runtime-approved zh-CN short actionable copy.
- Human-facing manual restore blockers MUST use runtime-approved zh-CN short actionable copy.

## Source Of Truth

- `workflow/runtime.schema.yaml`
- `workflow/router.spec.yaml`
- `workflow/policy.spec.yaml`
- `workflow/capabilities.yaml`
- `workflow/persona-bindings.yaml`
- `workflow/file-classes.yaml`
- `workflow/approval-matrix.yaml`
- `workflow/hooks.spec.yaml`
- `docs/architecture-v4.md`
- `.gemini/settings.json`

## Routing Summary

- Execution is state-first and blocker-first.
- V4 Phase 1 uses a single active phase and a single active node.
- Default review chain: `author` -> `spec_reviewer` -> `quality_reviewer`.
- `recommended_next` and `restore_reason` MUST come from runtime state, not free-form chat reasoning.

## Hooks And Approval

- Use native Gemini hooks from `.gemini/settings.json` as hard guards at `SessionStart`, `BeforeAgent`, `BeforeTool`, `AfterTool`, and `AfterAgent`.
- Keep `general.defaultApprovalMode = "default"` as the safe baseline; do not rely on `auto_edit` or `yolo` for normal workflow execution.
- Project hooks are fingerprinted by Gemini CLI; treat hook changes as trusted-project review boundaries.
- If runtime enters `approval_pending`, surface the runtime approval request in zh-CN short actionable copy and wait.

## Planning Mode

- During planning, design, or review-only phases, stay read-only unless the runtime explicitly enters an execution node.
- Do not write files, mutate schema, or run risky commands before router and policy gates allow execution.
- Use the workflow runtime state under `.ai/workflow/` as the execution boundary contract.

## Recovery And Checkpointing

- Keep Gemini checkpointing enabled so interrupted write operations can recover through `/restore`.
- If runtime enters `restore_pending`, resolve recovery before any new write or command.
- Resume from checkpointed state before proposing new writes after interruption.
- If manual intervention is required, surface the runtime restore request in zh-CN short actionable copy and stop.
- Treat runtime recovery state as authoritative over chat recap when the two diverge.

## Skills And Isolation

- Use skills progressively and load only the minimum files required for the active task.
- Keep reviewer personas isolated from author context.
- Do not bypass router, policy, or recovery boundaries through ad-hoc role switching.

## Skill Frontmatter

- `$ARGUMENTS` captures the command tail; `$0`, `$1`, ... map positional arguments.
- `disable-model-invocation: true` marks manual-only skills or commands.
- Honor `allowed-tools` as the maximum tool scope for a skill.
<!-- SY:GENERATED:END -->
<!-- SY:SEEDED:BEGIN -->
<!-- Add project-specific overrides below. -->
<!-- SY:SEEDED:END -->
