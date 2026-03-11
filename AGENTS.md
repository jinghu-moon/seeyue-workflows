<!-- SY:GENERATED:BEGIN {"generator":"seeyue-compile-adapter","version":1,"engine":"codex","pass":"routing","registry_revision":"2026-03-11","spec_hash":"616d60fe5a1f409646f9c0319c70fb30f40b3b7bf6b9006dc97f004dbaae498b"} -->
# AGENTS.md

> Generated artifact for `codex`. Do not edit manually.
> Vendor files are deployment artifacts. `workflow/*.yaml` remains the machine source of truth.

## Scope And Layering

- This file defines the root instruction layer for Codex.
- With `features.child_agents_md = true`, nested `AGENTS.md` files may add narrower instructions by directory scope.
- Prefer durable workflow state under `.ai/workflow/` over chat memory or free-form recap.

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
- `.codex/skill-metadata.json`

## Routing Summary

- Execution is state-first and blocker-first.
- V4 Phase 1 uses a single active phase and a single active node.
- Default review chain: `author` -> `spec_reviewer` -> `quality_reviewer`.
- `recommended_next` and `restore_reason` MUST come from runtime state, not free-form chat reasoning.

## Approval And Sandbox

- Use `approval_policy = "on-request"` and `sandbox_mode = "workspace-write"` as the minimum safe Codex profile.
- Destructive, git-mutating, privileged, schema-mutating, data-mutating, and sensitive network actions require human approval.
- Notify-only relief is limited to low-risk `docs`, `scaffold`, and `utility` changes after verification passes.
- If runtime enters `approval_pending`, surface the runtime approval request in zh-CN short actionable copy and wait.

## Recovery And Resume

- If runtime enters `restore_pending`, resolve recovery before any new write or command.
- If manual intervention is required, surface the runtime restore request in zh-CN short actionable copy and stop.
- Treat runtime recovery state as authoritative over chat recap when the two diverge.

## Skills

- Skill discovery metadata is compiled into `.codex/skill-metadata.json`.
- Load skills with progressive disclosure only: inspect metadata first, then open the exact `SKILL.md` required for the active task.
- Core workflow skills in this repository include: `sy-constraints`, `sy-executing-plans`, `sy-workflow`.
- Keep reviewer personas isolated from author context when invoking workflow or review skills.

## Skill Frontmatter

- `$ARGUMENTS` captures the command tail; `$0`, `$1`, ... map positional arguments.
- `disable-model-invocation: true` marks manual-only skills or commands.
- Honor `allowed-tools` as the maximum tool scope for a skill.
<!-- SY:GENERATED:END -->
