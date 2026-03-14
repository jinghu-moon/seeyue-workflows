# seeyue-workflows V4 Architecture Specification

Status: Formal review candidate  
Version: 4.0.0-rc1  
Audience: workflow maintainers, runtime authors, validators, agent adapters

## 1. Scope and Precedence

[RULE]
This document is the normative V4 architecture specification for `seeyue-workflows`.

[RULE]
This document defines the control-plane semantics for:
- workflow state
- event flow
- approval flow
- persona isolation
- constraint enforcement
- autonomous loop control
- engine adapter behavior

[RULE]
This document is written for agent/runtime input, not for end-user product presentation.

[RULE]
If this document conflicts with explanatory or research documents under `refer/`, this document takes precedence.

[RULE]
The following documents are informative, not normative:
- `seeyue-workflows/refer/workflow-skills-system-design-v3.md`
- `seeyue-workflows/refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md`

[RULE]
Future machine-readable specs such as `workflow/runtime.schema.yaml`, `workflow/router.spec.yaml`, and `workflow/policy.spec.yaml` MAY supersede this document for their narrow scope once they exist and are approved.

## 2. Language Partition

[RULE]
All normative rule text, persona contracts, protocol definitions, schemas, and RFC2119 statements MUST be written in English.

[RULE]
All human-facing approval, clarification, and status output MUST be written in zh-CN.

[RULE]
Code, command, path, tool name, environment variable, and schema field tokens MUST remain verbatim.

[RULE]
Normative text MUST use unambiguous wording.
Avoid vague wording such as "maybe", "probably", "roughly", "normally", "as needed", or "if possible" unless bounded by a stronger rule.

[RULE]
RFC keywords MUST remain uppercase: MUST, MUST NOT, SHOULD, SHOULD NOT, MAY.

[RULE]
`[RULE]` blocks are normative and English-only.
`[OUTPUT]` blocks are user-facing and zh-CN-only.
`[CODE]` blocks keep executable tokens verbatim.

## 3. System Objectives

[RULE]
V4 MUST transform `seeyue-workflows` from a workflow prompt collection into an engine-neutral workflow control plane.

[RULE]
V4 MUST optimize for:
- deterministic phase transitions
- durable machine-readable state
- resumable long-running execution
- task-isolated reasoning
- physical TDD enforcement
- short human approval interactions
- engine-neutral policy compilation

[RULE]
Chat history MUST NOT be treated as authoritative workflow state.

[RULE]
The runtime MUST prefer durable state over chat memory, evidence over claims, and validators over prompt optimism.

## 4. Architecture Overview

[RULE]
V4 consists of five logical subsystems:

1. Logical Specs
2. Durable Runtime Store
3. Execution Services
4. Constraint Layers
5. Engine Adapters

[RULE]
The Logical Specs define policy and routing semantics.
The Durable Runtime Store defines the current run, task graph, journal, checkpoints, and handoff artifacts.
The Execution Services implement routing, validation, compaction, approval bridging, and recovery.
The Constraint Layers enforce behavior boundaries.
The Engine Adapters translate V4 semantics into Claude Code, Codex, and Gemini CLI artifacts.

[RULE]
The runtime MUST be event-driven.
Every meaningful workflow transition MUST be representable as a durable event.

### 4.1 Design Lineage

[RULE]
V4 intentionally incorporates the following proven patterns:
- hierarchical instruction scoping and progressive disclosure, aligned with Codex-style instruction layering
- durable task state and plan-mode separation, aligned with Gemini-style state-over-chat discipline
- approval interrupts and hook-enforced boundaries, aligned with Claude Code-style event interception
- fresh subagent per task and spec-review-before-quality-review, aligned with Superpowers task isolation
- research-first, verification-first, and context-budget discipline, aligned with Everything-Claude-Code operating style
- root-cause-first debugging
- physical RED -> GREEN -> REFACTOR gates

### 4.2 Ratified V4 Decisions

[RULE]
V4 Phase 1 product scope MUST be an engine repository with minimal working adapters and hook surfaces.
It MUST NOT expand into a template-heavy monorepo during the initial rollout.

[RULE]
`workflow/*.yaml` MUST be treated as the machine source of truth for logical workflow policy.
`docs/*.md` MUST remain the human review and explanatory layer.

[RULE]
Phase 1 implementation priority MUST target `Claude Code` and `Codex` adapters first.
`Gemini CLI` support remains a required target architecture, but it SHOULD follow after the first stable adapter pair exists.

[RULE]
The Unified TDD Contract MUST be enforced as a hard gate by default.
Production writes before RED evidence MUST be blocked unless the change is `docs-only` or the human explicitly approves a `tdd-exception`.

[RULE]
Approval grants MUST use one of these scopes:
- `once`
- `session`

[RULE]
Approval mode MUST be classified as exactly one of:
- `manual_required`
- `never_auto`

[RULE]
Approval records MUST keep `grant_scope` separate from `approval_mode`.
The runtime SHOULD track `pending_count` so approval queue budgets can be enforced deterministically.

[RULE]
Persona isolation MUST begin as logical isolation through contracts, capsules, routing, and review chains.
Physical isolation MAY be added later, but it MUST NOT be a Phase 1 prerequisite.

## 5. Source of Truth Model

[RULE]
V4 MUST separate source of truth into three classes.

### 5.1 Logical Specs

[RULE]
The logical source of truth SHOULD be stored under a workflow spec directory and MUST eventually include:
- `workflow/router.spec.yaml`
- `workflow/policy.spec.yaml`
- `workflow/persona-bindings.yaml`
- `workflow/runtime.schema.yaml`
- `workflow/file-classes.yaml`
- `workflow/approval-matrix.yaml`

[RULE]
Logical Specs MUST be edited by maintainers and reviewed as architecture artifacts.

### 5.2 Durable Runtime Store

[RULE]
The durable runtime store MUST live under `.ai/workflow/` and MUST include:
- `session.yaml`
- `task-graph.yaml`
- `sprint-status.yaml`
- `journal.jsonl`
- `ledger.md`
- `capsules/`
- `checkpoints/`

[RULE]
The durable runtime store is the authoritative execution state for active and resumable work.

### 5.3 Engine Adapters

[RULE]
Vendor-specific files are deployment artifacts, not logical source of truth.

[RULE]
The adapter layer MUST be able to generate or maintain at least:
- `.claude/settings.json`
- `CLAUDE.md`
- `AGENTS.md`
- `GEMINI.md`
- engine-specific policy glue

### 5.4 Document-to-Machine Mapping

[RULE]
The following mapping table is informative for maintainers, but it MUST be kept aligned with the approved machine-readable specs.

| Architecture Concept | Canonical Machine Representation | Primary Source |
|---|---|---|
| Session identity | `session.run_id` | `workflow/runtime.schema.yaml` |
| Engine identity | `session.engine.kind`, `session.engine.adapter_version` | `workflow/runtime.schema.yaml` |
| Task identity | `session.task.id`, `session.task.title`, `session.task.mode` | `workflow/runtime.schema.yaml` |
| Current phase | `session.phase.current`, `session.phase.status` | `workflow/runtime.schema.yaml` |
| Active node | `session.node.active_id`, `session.node.state`, `session.node.owner_persona` | `workflow/runtime.schema.yaml` |
| Loop budget | `session.loop_budget.max_nodes`, `max_failures`, `max_pending_approvals`, `consumed_nodes`, `consumed_failures` | `workflow/runtime.schema.yaml` |
| Approval queue | `session.approvals.pending`, `pending_count`, `active_request`, `grants` | `workflow/runtime.schema.yaml` |
| Approval semantics | `approval_mode`, `grant_scope`, `risk_class`, `notify_only` eligibility | `workflow/runtime.schema.yaml`, `workflow/policy.spec.yaml` |
| Task topology | `task_graph.phases[]`, `task_graph.nodes[]` | `workflow/runtime.schema.yaml` |
| Node execution contract | `verify`, `capability`, `priority`, `condition`, `retry_policy`, `timeout_policy`, `test_contract` | `workflow/runtime.schema.yaml` |
| Resume frontier | `sprint_status.active_phase`, `sprint_status.node_summary`, `sprint_status.recommended_next` | `workflow/runtime.schema.yaml` |
| Router decision output | `recommended_next`, `route_basis`, `block_reason`, persona-capability routing | `workflow/router.spec.yaml`, `workflow/runtime.schema.yaml` |
| Event stream | `journal.event_shape.event`, `payload.route_decision`, `payload.route_basis` | `workflow/runtime.schema.yaml` |
| TDD and completion gates | `node_test_contract`, `tdd_contract`, `completion_gates`, `hook_enforcement` | `workflow/policy.spec.yaml` |
| Execution resilience | `retry_policy`, `timeout_policy`, `node_timed_out` routing semantics | `workflow/policy.spec.yaml`, `workflow/router.spec.yaml`, `workflow/runtime.schema.yaml` |
| Human approval output | short zh-CN request derived from `grant_scope`, `approval_mode`, `risk_class`, and target information | `workflow/policy.spec.yaml`, adapter outputs |

[RULE]
If this document and the mapping table diverge from machine-readable specs, the machine-readable specs take precedence for execution and validation.

## 6. Durable Runtime Store

### 6.1 Session State

[RULE]
`.ai/workflow/session.yaml` MUST describe the current run only.
It MUST NOT be treated as the full historical audit log.

[RULE]
`session.yaml` MUST contain at least:
- `schema`
- `run_id`
- `engine.kind`
- `engine.adapter_version`
- `task.id`
- `task.title`
- `task.mode`
- `phase.current`
- `phase.status`
- `node.active_id`
- `node.state`
- `node.owner_persona`
- `loop_budget`
- `context_budget`
- `workspace`
- `approvals`
- `recovery`
- `timestamps.created_at`
- `timestamps.updated_at`

[RULE]
The legal phase set is:
- `discover`
- `benchmark`
- `ideation`
- `design`
- `worktree`
- `plan`
- `execute`
- `debug`
- `verify`
- `review`
- `review-feedback`
- `complete`
- `done`

[RULE]
The legal phase status set is:
- `pending`
- `in_progress`
- `blocked`
- `review`
- `completed`

[RULE]
The legal node state set is:
- `idle`
- `red_pending`
- `red_verified`
- `green_pending`
- `green_verified`
- `refactor_pending`
- `verified`
- `failed`

### 6.2 Task Graph

[RULE]
`.ai/workflow/task-graph.yaml` MUST be the authoritative execution topology.
The runtime MUST NOT rely on markdown checklists in chat for multi-step state.

[RULE]
Each node MUST define:
- `id`
- `phase_id`
- `title`
- `target`
- `action`
- `why`
- `depends_on`
- `verify`
- `risk_level`
- `tdd_required`
- `status`
- `tdd_state`
- `owner_persona`
- `review_state`
- `evidence_refs`
- `output_refs`
- `capability`
- `priority`

[RULE]
Each node MAY declare optional fields such as `condition`, `parallel_group`, `approval_ref`, `retry_policy`, `timeout_policy`, and `test_contract`.
If declared, they MUST remain machine-readable and MUST NOT rely on free-form prose semantics.

[RULE]
`parallel_group` is RESERVED in Phase 1.
While V1 single-active execution is in effect, `parallel_group` MUST remain null and MUST NOT be used to imply concurrent scheduling.

[RULE]
The scheduler MUST derive `ready` nodes from dependency closure, not from conversational implication.

### 6.3 Sprint Status

[RULE]
`.ai/workflow/sprint-status.yaml` MUST summarize the current execution frontier for resume and handoff.

[RULE]
`sprint-status.yaml` MUST contain at least:
- `schema`
- `active_phase`
- `node_summary`
- `recommended_next`

[RULE]
`recommended_next` in `sprint-status.yaml` MUST use the same machine schema as `router.spec.yaml`.

### 6.4 Journal and Ledger

[RULE]
`.ai/workflow/journal.jsonl` MUST be the append-only machine audit stream.

[RULE]
Every durable workflow event MUST be representable as one journal row.

[RULE]
`ledger.md` MUST be a human-readable summary derived from durable events.
Hooks and validators SHOULD prefer `journal.jsonl` over markdown parsing.

### 6.5 Capsules

[RULE]
Each verified node MUST produce a capsule under `.ai/workflow/capsules/`.

[RULE]
A capsule MUST contain:
- node identity
- goal summary
- touched files
- interface changes
- RED/GREEN/VERIFY evidence summary
- active risks
- next handoff notes

[RULE]
Personas MUST consume capsules by default instead of full transcript replay.

### 6.6 Run Archival and Bootstrap

[RULE]
A new run MUST be created only when no active session exists or the active session is already in a clean terminal handoff state.

[RULE]
A clean terminal handoff state requires all of the following:
- `session.phase.status = completed`
- `session.node.active_id = none`
- `session.approvals.pending_count = 0`
- `session.recovery.restore_pending = false`

[RULE]
The runtime MUST refuse bootstrap when an active run remains executable, pending approval work exists, or recovery is still required.

[RULE]
Before a new run is initialized, the runtime MUST archive the previous active run under `.ai/archive/<run_id>/`.

[RULE]
The archive MUST preserve at least:
- `session.yaml`
- `task-graph.yaml`
- `sprint-status.yaml`
- `journal.jsonl`
- `ledger.md`
- `capsules/`
- `checkpoints/`
- key analysis artifacts required for audit continuity

[RULE]
The archive MUST include a machine-readable manifest with archived paths and archive timestamp.

[RULE]
Bootstrap MUST clear the active runtime area only after archival succeeds.

[RULE]
Bootstrap MUST then initialize a fresh active runtime state from a task graph template and MUST reset phase and node statuses deterministically.

[RULE]
Bootstrap MUST emit `session_started` and `phase_entered` as the first durable events of the new run.

[RULE]
Bootstrap MUST recompute `recommended_next` from the reset graph using the router item schema.

[RULE]
If bootstrap produces no ready node, the runtime MUST emit `human_intervention` rather than guessing the next executable step.

## 7. Event Model

[RULE]
The runtime MUST model workflow progression as durable events.

[RULE]
The minimum event set SHOULD include:
- `session_started`
- `phase_entered`
- `phase_completed`
- `node_started`
- `red_recorded`
- `green_recorded`
- `verification_recorded`
- `node_completed`
- `node_failed`
- `node_timed_out`
- `node_bypassed`
- `approval_requested`
- `approval_resolved`
- `approval_expired`
- `review_verdict_recorded`
- `checkpoint_created`
- `checkpoint_restored`
- `budget_exhausted`
- `session_stopped`
- `session_resumed`
- `validation_failed`

[RULE]
If a tool starts but no terminal event exists after interruption, recovery MUST synthesize an `aborted` terminal interpretation before resuming.

## 8. Autonomous Loop Budgets and Recovery

[RULE]
Every autonomous run MUST define loop budgets before entering `execute`.

[RULE]
Loop budgets MUST include:
- `max_nodes`
- `max_failures`
- `max_pending_approvals`

[RULE]
Context budgets MUST include:
- `strategy`
- `capsule_refresh_threshold`
- `summary_required_after_turns`

[RULE]
The runtime SHOULD track `pending_count` and MUST stop autonomous progression when `pending_count > max_pending_approvals`.

[RULE]
The runtime MAY include optional budgets such as `max_minutes`, `max_cost`, and `max_rework_cycles`.

[RULE]
Autonomous progression MUST stop when any mandatory budget is exceeded.

[RULE]
The runtime MUST enter `recovering` when:
- the process is interrupted during an active node
- a tool call starts but has no terminal record
- a pending approval survives restart
- `instruction_chain_hash` changes unexpectedly

[RULE]
Recovery MUST reconstruct current intent from:
- `session.yaml`
- `task-graph.yaml`
- `journal.jsonl`
- latest verify artifacts
- latest persona capsule

[RULE]
If `timeout_policy` triggers during execution, the runtime MUST emit `node_timed_out` before routing into retry, block, or human escalation.

### 8.1 Checkpoints

[RULE]
The runtime MUST support at least three checkpoint classes:
- node checkpoint
- review checkpoint
- pre-destructive checkpoint

[RULE]
A node checkpoint MUST be created after successful node verification.

[RULE]
A review checkpoint MUST be created after a review verdict is finalized.

[RULE]
A pre-destructive checkpoint MUST be created before destructive file-system or git mutations.

[RULE]
A checkpoint SHOULD capture:
- session snapshot
- diff snapshot or git reference
- relevant capsule snapshot

## 9. Context Continuity and Compaction

[RULE]
The runtime MUST use three context tiers:
- Hot Context
- Warm Context
- Cold Context

[RULE]
Hot Context MUST contain the active capsule id, evidence refs, and recommended_next.

[RULE]
Warm Context SHOULD contain task_id, verdict, and constraints.

[RULE]
Cold Context MUST contain journal_ref, checkpoints_dir, and capsules_dir.

[RULE]
Compaction MUST be evaluated by context-manager using:
- `context_utilization >= 0.80`
- `turns_since_summary >= session.context_budget.summary_required_after_turns`
- `turns_since_capsule >= session.context_budget.capsule_refresh_threshold`
- explicit `force`

[RULE]
The runtime SHOULD invoke compaction checks before long autonomous continuation; compaction is explicit (not implicit) and requires a context-manager call.

[RULE]
Compaction MUST generate or refresh a capsule and MUST reduce prompt load to the minimum data needed for the next step.

[RULE]
The runtime MUST prefer progressive disclosure for skills, operations, references, and examples.

## 10. The 4-Layer Constraint Architecture

### 10.1 L0 System Hooks

[RULE]
L0 is the hard enforcement layer.
It MUST be the final authority for write, shell, approval, and completion boundaries.

[RULE]
L0 MUST implement at least:
- `pre-write`
- `pre-bash`
- `post-write`
- `post-bash`
- `stop`
- optional `session-start`

[RULE]
`pre-write` MUST validate:
- current phase
- current node state
- target path class
- write scope
- TDD gate status
- debug gate status
- approval queue budget

[RULE]
Runtime state validation (phase/node/scope/TDD) applies only when runtime state is ready and the target is production code. For non-production writes, `pre-write` SHOULD still enforce protected-file, approval, secret, and debug gates.

[RULE]
`pre-bash` MUST classify commands into:
- safe
- verify
- destructive
- git_mutating (alias: git-mutating)
- network_sensitive (alias: networked)
- privileged
- schema_mutation
- data_mutation

[RULE]
Destructive, git_mutating, network_sensitive, privileged, schema_mutation, or data_mutation commands MUST require human approval.

[RULE]
`post-write` MUST append journal events and invalidate stale code understanding artifacts when required.

[RULE]
`post-bash` MUST extract RED, GREEN, and VERIFY evidence into machine-readable artifacts.

[RULE]
`stop` MUST block session completion if:
- approval is pending
- node verification is incomplete
- final review evidence is stale
- required state fields are missing

### 10.2 L1 Router

[RULE]
L1 is the deterministic routing layer.
It MUST choose the next skill and persona from durable state, not from chat intuition alone.

[RULE]
L1 MUST compile to engine-native routing artifacts such as `CLAUDE.md`, `AGENTS.md`, and `GEMINI.md`.

[RULE]
L1 MUST load only the minimum required skill set for the current turn.

[RULE]
L1 MUST prefer phase continuity over broad re-analysis when valid state already exists.

[RULE]
If user intent conflicts with an active safety or approval state, L1 MUST resolve safety first.

[RULE]
If `completion.stop_gate_ready = true` and no next phase or node is executable, L1 MUST emit a terminal human handoff:
- `recommended_next[0].type = human_intervention`
- `recommended_next[0].target = session`
- emit `session_stopped`
- runtime MUST persist a clean terminal state before returning control to the human

### 10.3 L2 Personas

[RULE]
L2 is the isolated reasoning layer.
Personas MUST have distinct tool permissions, input contracts, and output contracts.

[RULE]
V4 MUST define at least these personas:
- Planner
- Author
- Reviewer
- Reader
- Auditor

[RULE]
Reviewer MUST be split logically into:
- Spec Reviewer
- Quality Reviewer

[RULE]
Each persona invocation MUST receive a capsule-scoped input package.
It MUST NOT depend on full transcript replay by default.

### 10.4 L3 Validation

[RULE]
L3 is the machine validation layer.
It MUST verify state integrity, report freshness, phase legality, node legality, and evidence completeness.

[RULE]
L3 MUST produce a four-level verdict:
- `PASS`
- `CONCERNS`
- `REWORK`
- `FAIL`

[RULE]
`PASS` allows progression.
`CONCERNS` allows progression with explicit risk carry-forward.
`REWORK` forces return to `review-feedback` or the current node.
`FAIL` stops autonomous progression and requires human intervention.

## 11. Persona Contracts

### 11.1 Planner

[RULE]
Planner MUST be read-only with respect to source code.
Planner MAY write plan artifacts only.

[RULE]
Planner output MUST include:
- objective
- scope
- impacted files or modules
- phased implementation plan
- verification strategy
- rollback strategy

### 11.2 Author

[RULE]
Author MAY write only within the current node write scope.

[RULE]
Author input MUST include the full node text, acceptance criteria, current design summary, and explicit verify commands.

### 11.3 Reviewer

[RULE]
Spec Reviewer MUST validate "built the correct thing" before Quality Reviewer validates "built it well".

[RULE]
Reviewer MUST NOT trust implementer self-report without code inspection and evidence inspection.

### 11.4 Reader

[RULE]
Reader MUST produce structure and dependency understanding only.
Reader MUST NOT silently drift into implementation planning.

### 11.5 Auditor

[RULE]
Auditor MUST verify claims against files, state, and command evidence.
Auditor MUST mark unsupported claims as `unverified`.

## 12. Task-Isolated Execution Protocol

[RULE]
Every implementation node SHOULD be executed by a fresh task-scoped persona invocation.

[RULE]
The active task text MUST be passed directly into the persona input package.
The runtime SHOULD NOT require the implementer persona to rediscover the task from large plan files.

[RULE]
The minimum isolated execution chain is:
1. Author
2. Spec Reviewer
3. Quality Reviewer

[RULE]
The runtime MUST NOT mark a node complete until the required review chain succeeds.

## 13. Unified TDD Contract

[RULE]
The Unified TDD Contract is mandatory for all feature work, bug fixes, and behavior changes unless the human explicitly approves an exception.

[RULE]
The TDD state sequence is:
- `red_pending`
- `red_verified`
- `green_pending`
- `green_verified`
- `refactor_pending`
- `verified`

[RULE]
Production code MUST NOT be written while the current node is `red_pending`.

[RULE]
During `red_pending`, only the following writes are allowed:
- test files
- plan files
- design files
- `.ai/workflow/*`

[RULE]
If production code is detected before RED evidence exists, `pre-write` MUST block the write.

[RULE]
RED evidence MUST include:
- exact command
- non-zero exit code or expected failing signal
- failure reason consistent with missing behavior

[RULE]
GREEN evidence MUST include the target test passing.

[RULE]
VERIFY evidence MUST include the node-level verification commands passing.

[RULE]
Bug fixes MUST begin with a failing reproducer or characterization test.

[RULE]
If production code was written before RED evidence, the runtime MUST treat that state as invalid and MUST require rollback or overwrite through a compliant TDD path.

## 14. Research-First and Debug-First Gates

[RULE]
Before introducing a new abstraction, helper, dependency, or integration, the agent MUST perform an `adopt | extend | build` evaluation.

[RULE]
The minimum research sweep SHOULD cover:
- local repository reuse
- official documentation
- package registry candidates
- MCP or built-in platform capabilities
- relevant workflow skills already available

[RULE]
When verify, review, build, test, or runtime failures recur, the runtime MUST route to `debug`.

[RULE]
The debug workflow MUST require root cause confirmation before repair writes are allowed.

## 15. Human Approval and Human Output Contract

[RULE]
Human-facing approval text MUST be short, explicit, and action-oriented.
It MUST NOT contain long architectural explanations.

[RULE]
Every approval request MUST tell the human:
- what the agent wants to do
- what grant scope will be issued if approved
- what the approval mode is
- what the risk is
- what decision is expected

[RULE]
Human-facing workflow status text SHOULD be concise enough to scan in a few seconds.
Approval requests SHOULD fit within four short lines whenever possible.



[OUTPUT]
推荐审批格式：

需要审批：<动作>
授权：<单次 / 本次会话>
模式：<人工审批 / 永不自动批准>
范围：<文件 / 命令 / 目录 / 网络目标>
风险：<一句话风险说明>
请选择：批准 / 拒绝 / 本次会话内批准

[OUTPUT]
推荐阻塞提示：

当前不能继续：<原因>
需要你决定：<下一步选择>

[OUTPUT]
示例：

需要审批：执行 `git commit`
授权：单次
模式：人工审批
范围：当前工作区已暂存改动
风险：会生成新的提交记录
请选择：批准 / 拒绝

## 16. Engine Adapter Contract

[RULE]
V4 MUST support engine adapters for Claude Code, Codex, and Gemini CLI.

[RULE]
The adapter layer MUST translate the logical workflow model into engine-native artifacts without changing the logical policy.

[RULE]
The adapter layer SHOULD map:
- approval model
- sandbox model
- context file format
- skill registration format
- hook or policy integration points

[RULE]
If an engine lacks a native hook surface for a specific rule, the adapter MUST enforce that rule at the nearest stronger boundary available.

### 16.1 Claude Code Adapter

[RULE]
The Claude Code adapter SHOULD target hook events, settings files, and `CLAUDE.md` routing behavior.

### 16.2 Codex Adapter

[RULE]
The Codex adapter SHOULD target `AGENTS.md`, approval flow integration, sandbox policy integration, and skill loading behavior.

### 16.3 Gemini CLI Adapter

[RULE]
The Gemini adapter SHOULD target policy engine rules, `GEMINI.md`, checkpoint/resume semantics, and read-only planning mode semantics.

## 17. Formal Rollout Order

[RULE]
The implementation order for V4 SHOULD be:

1. freeze logical specs
2. define runtime schemas
3. implement validators
4. implement journal and recovery
5. implement L0 policy kernel
6. implement engine adapters
7. implement persona capsules
8. implement Unified TDD physical gates
9. rewrite workflow skills against the new runtime
10. add simulation and regression harnesses

## 18. Acceptance Criteria

[RULE]
V4 is ready for adoption review only when all of the following are true:
- the runtime can resume safely after interruption
- the runtime can block production writes before RED evidence
- the runtime can express approvals in short zh-CN output
- the runtime can route through isolated personas
- the runtime can produce deterministic `recommended-next`
- the runtime can compile to Claude Code, Codex, and Gemini adapters
- the runtime can survive long autonomous loops without using chat as task state
- the runtime can archive a stopped run and bootstrap a new run without leaking prior active state

## 19. Non-Goals

[RULE]
V4 does not attempt to:
- replace vendor-native model APIs
- build a GUI dashboard first
- preserve every V3 implementation shortcut
- allow free-form autonomous execution without budgets or approvals

