<!-- SY:GENERATED:BEGIN {"generator":"seeyue-compile-adapter","version":1,"engine":"claude_code","pass":"routing","registry_revision":"2026-03-11","spec_hash":"616d60fe5a1f409646f9c0319c70fb30f40b3b7bf6b9006dc97f004dbaae498b"} -->
# CLAUDE.md

> Generated artifact for `claude_code`. Do not edit manually.
> Vendor files are deployment artifacts. `workflow/*.yaml` remains the machine source of truth.

## Mission

- Use this repository as the runtime and adapter source for `seeyue-workflows`.
- Prefer durable workflow state under `.ai/workflow/` over chat memory or free-form recap.
- Keep context narrow and load only the minimum skill or document scope needed for the active node.

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

## Router Summary

- Execution is state-first and blocker-first.
- V4 Phase 1 uses a single active phase and a single active node.
- Default review chain: `author` -> `spec_reviewer` -> `quality_reviewer`.
- `recommended_next` and `restore_reason` MUST come from runtime state, not free-form chat reasoning.

## Approval Summary

- Human approval is required for command classes: `destructive`, `git_mutating`, `network_sensitive`, `privileged`, `schema_mutation`, `data_mutation`.
- Human approval is required for file classes: `system_file`, `security_boundary`, `secret_material`, `critical_policy_file`.
- Notify-only relief is limited to low-risk change classes: `docs`, `scaffold`, `utility`.
- If runtime enters `approval_pending`, surface the runtime approval request in zh-CN short actionable copy and wait.

## Hook Summary

- `SessionStart`: bootstrap workflow + constraint routing.
- `UserPromptSubmit`: refresh the long-session anchor.
- `PreToolUse(Bash)`: command class approval and loop-budget guard.
- `PreToolUse(Write|Edit)`: TDD, secret, protected-file, and session-integrity guard.
- `PostToolUse(Write|Edit)`: capture write evidence and scope drift.
- `PostToolUse(Bash)`: capture verification and TDD evidence.
- `Stop`: checkpoint and resume-frontier gate.

## Recovery Summary

- If runtime enters `restore_pending`, resolve recovery before any new write or command.
- If manual intervention is required, surface the runtime restore request in zh-CN short actionable copy and stop.
- Treat runtime recovery state as authoritative over chat recap when the two diverge.

## Skills And Personas

- Load skills from `.agents/skills` with progressive disclosure only.
- Keep reviewer personas isolated from the author context.
- Do not bypass router, policy, or hook boundaries through ad-hoc role switching.

## Skill Frontmatter

- `$ARGUMENTS` captures the command tail; `$0`, `$1`, ... map positional arguments.
- `disable-model-invocation: true` marks manual-only skills or commands.
- Honor `allowed-tools` as the maximum tool scope for a skill.
<!-- SY:GENERATED:END -->
