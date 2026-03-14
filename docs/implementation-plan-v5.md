# V5 Implementation Plan (Engine-Neutral Control Plane)

Status: draft  
Scope: Move `seeyue-workflows` from V4 specifications to V5 executable control plane  
Plan type: execution roadmap only (no implementation in this document)

---

## 1. Baseline Snapshot

Machine specs already present:

- `workflow/runtime.schema.yaml`
- `workflow/router.spec.yaml`
- `workflow/policy.spec.yaml`
- `workflow/capabilities.yaml`
- `workflow/persona-bindings.yaml`
- `workflow/file-classes.yaml`
- `workflow/approval-matrix.yaml`
- `workflow/skills.spec.yaml`
- `workflow/hook-contract.schema.yaml`
- `workflow/validate-manifest.yaml`
- `workflow/output-templates.spec.yaml`
- `workflow/hooks.spec.yaml`
- `docs/architecture-v4.md`
- `docs/architecture-v5-proposal.md`

Execution status (as-is in this repo):

- P1-P5 主要节点已落地（详见 `.ai/workflow/task-graph.yaml`）。
- 本计划保留为执行记录与回溯参照，新增工作请以现行 specs 与 scripts 为准。

Known gaps for V5 (resolved in repo):

- Hook contract 版本强校验已补齐（Hook Client 启动前强制校验）。
- generated/seeded 边界的强制校验与 seeded 保留已补齐（Adapter 验证 + seeded 合并）。
- spec freeze gate 的强制流程化约束已补齐（Hook Client / Adapter 编译前强制校验）。

---

## 2. V5 Principles (Non-Negotiable)

1. **Thin hooks / Fat kernel**: hooks only parse stdin, call hook client, translate verdict.
2. **Engine-neutral skills**: skills defined once in `skills.spec.yaml`, compiled per engine.
3. **Progressive disclosure**: context files only contain skill stubs, not full bodies.
4. **Three output classes**: routing / skill / policy artifacts must be separate.
5. **Journal append safety**: O_APPEND (POSIX) + queue (Win32).
6. **Spec freeze gates**: no implementation begins unless required specs are frozen.
7. **Output contract**: outputs must conform to templates and be logged/validated.
8. **Review order**: spec review before quality review is enforced.
9. **TDD iron law**: block production writes without verified RED.
10. **Hook event matrix**: canonical events + engine availability + gap report.
11. **Skill refresh**: skills-manifest + change notifications.

---

## 3. Phase P1 — Spec Freeze + Runtime Store Foundations

### P1-N0: Create validation manifest
- **title**: Register all logical specs in a manifest
- **target**: `workflow/validate-manifest.yaml`
- **action**: Define spec entries, schemas, status (`draft`/`frozen`), freeze gates, and cross-refs (including output-templates).
- **depends_on**: `[]`
- **verify.cmd**: `node scripts/runtime/validate-specs.cjs --manifest`
- **risk_level**: low

### P1-N1: Add Skills Registry
- **title**: Introduce `workflow/skills.spec.yaml`
- **target**: `workflow/skills.spec.yaml`
- **action**: Define skill metadata, trigger predicates, args schema, engine overrides, allowed_tools, disable_model_invocation, output_template references, operations + references split (L1/L2).
- **depends_on**: `[P1-N0]`
- **verify.cmd**: `node scripts/runtime/validate-specs.cjs --spec workflow/skills.spec.yaml`
- **risk_level**: medium

### P1-N2: Add Hook Contract schema
- **title**: Introduce `workflow/hook-contract.schema.yaml`
- **target**: `workflow/hook-contract.schema.yaml`
- **action**: Define hook input/output envelopes, verdict enum, approval request schema, journal event schema, translation table, input_mutation constraints, stdout JSON-only contract, SessionStart non-blocking semantics, Stop force_continue semantics.
- **depends_on**: `[P1-N0]`
- **verify.cmd**: `node scripts/runtime/validate-specs.cjs --spec workflow/hook-contract.schema.yaml`
- **risk_level**: medium

### P1-N3: Manifest-driven spec validator
- **title**: Validate specs via manifest + enforce freeze gates
- **target**: `scripts/runtime/validate-specs.cjs`, `tests/runtime/spec-fixtures/`
- **action**: Manifest-driven validation, `SPEC_NOT_REGISTERED` warning, `SPEC_NOT_FROZEN_AT_REQUIRED_GATE` error, cross-ref validation (skills ↔ output templates).
 - **depends_on**: `[P1-N0, P1-N1, P1-N2, P1-N6]`
- **verify.cmd**: `node tests/runtime/run-spec-fixtures.cjs`
- **risk_level**: medium

### P1-N4: Journal append safety
- **title**: Replace read-concat-write in `appendJournalEvents`
- **target**: `scripts/runtime/store.cjs`
- **action**: O_APPEND for POSIX, queue/mutex for Win32, enforce <4 KB per line.
- **depends_on**: `[P1-N3]`
- **verify.cmd**: `node tests/runtime/run-runtime-store.cjs --case journal-append-lock-stale-recovery`
- **risk_level**: high

### P1-N5: Runtime store + checkpoint base
- **title**: Ensure durable store + checkpoint helpers are stable
- **target**: `scripts/runtime/store.cjs`, `scripts/runtime/checkpoints.cjs`
- **action**: Maintain atomic YAML writes, introduce checkpoint metadata and restore frontier.
- **depends_on**: `[P1-N4]`
- **verify.cmd**: `node tests/runtime/run-checkpoint-fixtures.cjs`
- **risk_level**: high

### P1-N6: Output template registry
- **title**: Introduce `workflow/output-templates.spec.yaml`
- **target**: `workflow/output-templates.spec.yaml`
- **action**: Define template ids, required variables, output levels, and i18n keys; register in manifest.
- **depends_on**: `[P1-N0]`
- **verify.cmd**: `node scripts/runtime/validate-specs.cjs --spec workflow/output-templates.spec.yaml`
- **risk_level**: low

### P1-N7: Hook event matrix spec
- **title**: Expand `workflow/hooks.spec.yaml` with event coverage + engine availability
- **target**: `workflow/hooks.spec.yaml`
- **action**: Declare canonical events (including BeforeToolSelection/AfterModel), per-engine availability, stdout JSON-only requirement, and capability-gap mapping output; register in manifest and freeze before P3 entry.
- **depends_on**: `[P1-N0]`
- **verify.cmd**: `node scripts/runtime/validate-specs.cjs --spec workflow/hooks.spec.yaml`
- **risk_level**: low

### P1-N8: Policy mode mapping spec
- **title**: Approval mode mappings for engines
- **target**: `workflow/policy.spec.yaml`
- **action**: Add explicit mode mappings (Gemini default/autoEdit/plan/yolo, Codex approval_policy, Claude managed settings), plus expected policy-tier mapping.
- **depends_on**: `[P1-N0]`
- **verify.cmd**: `node scripts/runtime/validate-specs.cjs --spec workflow/policy.spec.yaml`
- **risk_level**: low

---

## 4. Phase P2 — Router + Policy Kernel

### P2-N1: Pure Router
- **title**: Deterministic `recommended_next` and route verdicts
- **target**: `scripts/runtime/router.cjs`
- **depends_on**: `[P1-N5]`
- **verify.cmd**: `node tests/router/run-router-fixtures.cjs`
- **risk_level**: high

### P2-N2: Policy Kernel
- **title**: Unified gate evaluation (approval/test/coverage/timeout)
- **target**: `scripts/runtime/policy.cjs`
- **action**: Enforce loop budgets (max_nodes/max_failures/max_pending_approvals), stop gate preconditions, TDD iron-law gate, and review gating.
- **depends_on**: `[P1-N5]`
- **verify.cmd**: `node tests/policy/run-policy-fixtures.cjs`
- **risk_level**: critical

### P2-N3: Engine Kernel
- **title**: Compose router + policy output into a single verdict surface
- **target**: `scripts/runtime/engine-kernel.cjs`
- **depends_on**: `[P2-N1, P2-N2]`
- **verify.cmd**: `node tests/runtime/run-engine-kernel.cjs`
- **risk_level**: high

---

## 5. Phase P3 — Hook Client + Thin Hooks

**Entry condition**: `workflow/hook-contract.schema.yaml` and `workflow/hooks.spec.yaml` are frozen.

### P3-N1: Hook Client
- **title**: Normalize engine input + load snapshot + call kernel
- **target**: `scripts/runtime/hook-client.cjs`
- **action**: Build hook input/output envelopes, enforce single-read stdin contract, enforce stdout JSON-only contract, append journal via safe writer, add stop-loop guard.
- **depends_on**: `[P2-N3]`
- **verify.cmd**: `node tests/hooks/run-v4-fixtures.cjs --case hook-client-envelope`
- **risk_level**: high

### P3-N5: Hook capability-gap report
- **title**: Emit hook event availability report per engine
- **target**: `scripts/runtime/hook-client.cjs`, `scripts/adapters/compile-adapter.cjs`
- **action**: Generate a capability-gap report derived from hooks.spec.yaml and adapter outputs; store under `.ai/workflow/capability-gap.json`.
- **depends_on**: `[P3-N1, P1-N7]`
- **verify.cmd**: `node tests/hooks/run-v4-fixtures.cjs --case hook-gap-report`
- **risk_level**: medium

### P3-N2: Pre hooks refactor
- **title**: `pre-write` / `pre-bash` / `stop` as thin wrappers
- **target**: `scripts/hooks/sy-pretool-write.cjs`, `scripts/hooks/sy-pretool-bash.cjs`, `scripts/hooks/sy-stop.cjs`
- **action**: Remove direct runtime reads, call hook client, translate verdict, guarantee stdin single-read + stdout JSON-only, and Stop semantics.
- **depends_on**: `[P3-N1]`
- **verify.cmd**: `node tests/hooks/run-v4-fixtures.cjs --case prewrite-red-gate && node tests/hooks/run-v4-fixtures.cjs --case stop-requires-resume-frontier`
- **risk_level**: critical

### P3-N3: Post hooks refactor
- **title**: Evidence capture via hook client only
- **target**: `scripts/hooks/sy-posttool-write.cjs`, `scripts/hooks/sy-posttool-bash-verify.cjs`
- **action**: Journal append only via hook client; no direct store writes.
- **depends_on**: `[P3-N1]`
- **verify.cmd**: `node tests/hooks/run-v4-fixtures.cjs --case postwrite-journal-append`
- **risk_level**: high

### P3-N4: Session hooks normalization
- **title**: SessionStart must be non-blocking
- **target**: `scripts/hooks/sy-session-start.cjs`
- **action**: Ensure exit 2 only surfaces warnings and never blocks; wrap with hook client.
- **depends_on**: `[P3-N1]`
- **verify.cmd**: `node tests/hooks/run-v4-fixtures.cjs --case sessionstart-nonblocking`
- **risk_level**: medium

---

## 6. Phase P4 — Adapter Compiler (Routing / Skill / Policy)

**Entry condition**: `workflow/skills.spec.yaml` is frozen.

### P4-N1: Compiler three-pass pipeline
- **title**: Stub pass + Skill pass + Policy pass
- **target**: `scripts/adapters/compile-adapter.cjs`
- **action**: Generate routing stubs, skill files (with output template ids), policy artifacts separately.
- **depends_on**: `[P3-N3]`
- **verify.cmd**: `node tests/adapters/run-adapter-snapshots.cjs --engine claude_code`
- **risk_level**: high

### P4-N2: Generated/Seeded boundaries + drift detection
- **title**: Generated markers + verify-adapter
- **target**: `scripts/adapters/verify-adapter.cjs`, adapter templates
- **action**: Enforce generated sections, report drift.
- **depends_on**: `[P4-N1]`
- **verify.cmd**: `node tests/adapters/run-adapter-snapshots.cjs --engine codex`
- **risk_level**: medium

### P4-N3: Claude + Codex outputs
- **title**: Produce CLAUDE.md / AGENTS.md + policy artifacts
- **target**: `scripts/adapters/claude-code.cjs`, `scripts/adapters/codex.cjs`
- **depends_on**: `[P4-N1]`
- **verify.cmd**: `node tests/adapters/run-adapter-snapshots.cjs --engine claude_code && node tests/adapters/run-adapter-snapshots.cjs --engine codex`
- **risk_level**: medium

### P4-N4: Skills manifest + hot reload
- **title**: Emit skills manifest and change detection hooks
- **target**: `scripts/adapters/compile-adapter.cjs`, `scripts/runtime/skills-manifest.cjs`
- **action**: Generate `skills-manifest.json` with spec hash/version; provide change detection flow (Codex skills/changed → recompile; Gemini/Claude manual refresh hooks).
- **depends_on**: `[P4-N1]`
- **verify.cmd**: `node tests/adapters/run-adapter-snapshots.cjs --case skills-manifest`
- **risk_level**: medium

---

## 7. Phase P5 — Gemini Policy Engine + Recovery

### P5-N1: Gemini policy TOML generation
- **title**: Use policy engine for enforcement, hooks for evidence only
- **target**: `scripts/adapters/gemini-cli.cjs`
- **action**: Compile allow/deny/ask_user rules and map approval modes (default/autoEdit/plan/yolo) to Gemini policy tiers.
- **depends_on**: `[P4-N1]`
- **verify.cmd**: `node tests/adapters/run-adapter-snapshots.cjs --engine gemini_cli`
- **risk_level**: medium

### P5-N2: Recovery bridge
- **title**: Align checkpoint / resume semantics with runtime store
- **target**: `scripts/runtime/recovery-bridge.cjs`
- **action**: Enforce pre-write checkpoint semantics and restore frontier alignment.
- **depends_on**: `[P1-N5]`
- **verify.cmd**: `node tests/runtime/run-recovery-fixtures.cjs`
- **risk_level**: high

### P5-N3: Capsule + compaction manager
- **title**: Context continuity + resume frontier
- **target**: `scripts/runtime/context-manager.cjs`
- **depends_on**: `[P5-N2]`
- **verify.cmd**: `node tests/runtime/run-context-fixtures.cjs`
- **risk_level**: high

---

## 8. Phase P6 — Conformance + Release

### P6-N1: Engine conformance matrix
- **title**: Ensure stop semantics + approval gates + progressive disclosure match across engines
- **target**: `tests/e2e/engine-conformance/`
- **action**: Add cases for stop force_continue, stdout JSON-only hooks, progressive disclosure, review order, and output template compliance.
- **depends_on**: `[P4-N3, P5-N1]`
- **verify.cmd**: `node tests/e2e/run-engine-conformance.cjs --all`
- **risk_level**: critical

### P6-N2: Documentation + runbook updates
- **title**: Update adoption/operations/release docs
- **target**: `docs/*`
- **depends_on**: `[P6-N1]`
- **verify.cmd**: `node tests/e2e/run-doc-link-check.cjs`
- **risk_level**: low

### P6-N3: Release + sync
- **title**: Versioning and release artifacts
- **target**: `CHANGELOG.md`, `sync-manifest.json`
- **depends_on**: `[P6-N1]`
- **verify.cmd**: `node tests/e2e/run-release-fixtures.cjs`
- **risk_level**: medium

---

## 9. Phase P7 — Output Contract & Reporting

### P7-N1: Output template validator
- **title**: Validate template ids, variables, and levels
- **target**: `scripts/runtime/validate-output.cjs`, `tests/output/`
- **depends_on**: `[P1-N6]`
- **verify.cmd**: `node tests/output/run-output-template-fixtures.cjs`
- **risk_level**: medium

### P7-N2: Output log writer
- **title**: Append-only output.log for replay
- **target**: `scripts/runtime/output-log.cjs`, `.ai/workflow/output.log`
- **depends_on**: `[P7-N1]`
- **verify.cmd**: `node tests/output/run-output-log-fixtures.cjs`
- **risk_level**: low

### P7-N3: Skill output contract alignment
- **title**: Ensure skills reference templates + start announcements
- **target**: `workflow/skills.spec.yaml`, skill operation files
- **depends_on**: `[P1-N1, P7-N1]`
- **verify.cmd**: `node tests/output/run-output-contract-fixtures.cjs`
- **risk_level**: medium

---

## 10. Parallelization Rules

Only allow parallel execution for:

- `P4-N3` (Claude + Codex adapters)
- `P6-N2` + `P6-N3` (release hardening)

All other nodes are serial.

---

## 11. First Execution Batch (Minimum Risk)

Start with:

1. `P1-N0` validate manifest
2. `P1-N1` skills registry
3. `P1-N2` hook contract schema
4. `P1-N7` hook event matrix
5. `P1-N8` policy mode mapping
6. `P1-N3` manifest-driven validator
7. `P1-N6` output templates spec
8. `P1-N4` journal append fix

Rationale: these are the lowest-level invariants; everything else depends on them.

---

*End of Plan*
