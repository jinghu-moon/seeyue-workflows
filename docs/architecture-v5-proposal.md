# seeyue-workflows V5 Architecture Proposal

Status: draft  
Audience: workflow maintainers, runtime authors, validator authors, adapter compiler owners  
Baseline: `docs/architecture-v4.md` v4.0.0-rc1  
Sources: `refer/skills-and-hooks-architecture-advisory.md`, `refer/v4-architecture-update-proposal.md`, `refer/v4-architecture-patch-risks.md`, `refer/seeyue-workflow-Advanced-Agent-Engine-Architecture.md`, `refer/workflow-skills-system-design.md`, `refer/workflow-skills-system-design-v3.md`, `refer/workflow-skills-corrections.md`, `refer/phase5-integration-summary.md`, `refer/output-templates-reference.md`

---

## 0. 摘要

V5 是对 V4 的结构性加固，而非重写。核心目标是把 “Hooks 与 Skills 的编译链条、IPC 契约、并发安全、冻结门控” 从设计理念变成**机器可验证的架构不变量**，并把跨引擎差异收敛为统一的“Routing/Skill/Policy 三类产物”编译管线。

### 0.1 目标与范围

- 目标：形成可验证的控制平面规范（hook 合约、skills 机器源、冻结门控、编译产物边界），并让适配器输出稳定可复现。
- 目标：以“薄 Hook / 厚内核”模型统一 Claude/Codex/Gemini 的边界拦截行为，减少逻辑漂移。
- 范围：V5 只强化 V4 的子系统分层、规范冻结与编译链路，不重新设计 persona 或 UI。

### 0.2 V4 → V5 差异摘要

- 新增 Skills Registry（machine source），不再把技能当作适配器输出物。
- Hook 体系从“单层脚本”升级为“脚本 → Hook Client → Policy Kernel”三层内核化。
- 引入 validate-manifest 与冻结门控，使“何时可实现”具备机器化约束。
- 明确三类编译产物（routing/skill/policy）与 generated/seeded 边界。

### 0.3 当前落地情况（以仓库实现为准）

- 已落地：`workflow/skills.spec.yaml`、`workflow/hook-contract.schema.yaml`、`workflow/validate-manifest.yaml`、`workflow/output-templates.spec.yaml`；Hook Client + thin hooks；journal append 安全；输出模板校验与 output.log 持久化；hook capability-gap 报告。
- 已落地：Hook contract 版本强校验（Hook Client 启动前阻断）；generated/seeded 边界强制校验与 seeded 保留；spec freeze gate 已在 Hook Client 与 Adapter 编译流程中强制执行。

---

## 1. 核心不变量

[RULE]
V5 MUST treat Skills as a logical subsystem (Skills Registry) and the single source of truth for all engine skill artifacts.

[RULE]
V5 MUST implement a three-layer hook architecture: hook script (physical boundary), hook client (IPC + snapshot + journal), policy kernel (pure decision).

[RULE]
V5 MUST make journal append-only with OS-level atomic append semantics; read-before-write is prohibited for `journal.jsonl`.

[RULE]
V5 MUST compile **routing**, **skill**, and **policy** artifacts as distinct outputs and must not mix them in a single output stream.

[RULE]
V5 MUST enforce freeze gates for specs before any dependent implementation phase starts.

[RULE]
V5 MUST validate specs via a manifest file with status and freeze_gate; unregistered specs MUST emit SPEC_NOT_REGISTERED.

[RULE]
V5 MUST version hook-contract.schema.yaml and reject hook-client initialization on version mismatch.

[RULE]
V5 MUST classify policy artifacts as generated or seeded, and enforce protected markers for seeded sections.

[RULE]
V5 MUST provide a machine-readable engine-gap mapping table to drive adapter downgrade behavior.

---

## 2. Subsystem Layout (V5)

```
1. Logical Specs (machine source of truth)
   - router.spec.yaml
   - policy.spec.yaml
   - runtime.schema.yaml
   - persona-bindings.yaml
   - file-classes.yaml
   - approval-matrix.yaml
   - skills.spec.yaml            (NEW, required)
   - hook-contract.schema.yaml   (NEW, required)
   - validate-manifest.yaml      (NEW, required)
   - output-templates.spec.yaml  (NEW, required)

2. Durable Runtime Store (.ai/workflow)
   - session.yaml
   - task-graph.yaml
   - sprint-status.yaml
   - journal.jsonl
   - ledger.md
   - capsules/
   - checkpoints/

3. Execution Services
   - router (pure)
   - policy-kernel (pure)
   - validators
   - hook-client
   - journal writer
   - checkpoint manager

4. Constraint Layers
   - L0 hooks
   - L1 router
   - L2 personas
   - L3 validators

5. Engine Adapters
   - adapter compiler (routing/skill/policy passes)
   - per-engine output renderers
```

---

## 3. Logical Specs Additions

[RULE]
`workflow/skills.spec.yaml` MUST define skill metadata, trigger conditions, invocation policy, arguments schema, and engine overrides. Instruction bodies MUST be referenced, not inlined.

[RULE]
`workflow/hook-contract.schema.yaml` MUST define hook input/output envelopes, verdict enum, approval request schema, journal event schema, and engine translation table.

[RULE]
`workflow/validate-manifest.yaml` MUST register every Logical Spec file with status (`draft`/`frozen`) and a freeze gate (e.g., P3-N1, P4-N1). Unregistered specs MUST emit `SPEC_NOT_REGISTERED`.

[RULE]
Each spec MUST declare a `schema_version`. A spec is frozen only when `status: frozen` and `schema_version` is non-draft.

[RULE]
The manifest MUST declare cross_refs; validator MUST error on unresolved cross references.

[RULE]
`workflow/hooks.spec.yaml` MUST be frozen before Hook Client implementation begins.

[RULE]
`workflow/output-templates.spec.yaml` MUST define template ids, required variables, output level, and i18n keys. Skills MUST reference template ids instead of embedding raw output text.

[RULE]
`workflow/hooks.spec.yaml` MUST enumerate the canonical hook event set and provide an engine availability matrix. Adapters MUST map unsupported events to the nearest stronger boundary or explicitly mark them unsupported.

[RULE]
`workflow/policy.spec.yaml` MUST declare approval modes and their engine mappings (e.g., Gemini `default/autoEdit/plan/yolo`, Codex `approval_policy`, Claude managed settings), so policy artifacts are deterministic across engines.

[RULE]
`workflow/skills.spec.yaml` MUST include a `spec_hash` or versioned `revision` field for change detection. Adapter compiler MUST emit a `skills-manifest.json` containing registry hash, timestamp, and generated artifact list.

---

## 4. Hook Architecture (Thin Hooks / Fat Kernel)

[RULE]
Hook scripts MUST NOT read runtime state or evaluate policy. They only parse stdin, call Hook Client, translate verdict, and exit.

[RULE]
Hook Client MUST normalize engine input to the V4 hook envelope, assemble a read-only runtime snapshot, call policy kernel, append journal entries, and return a V4 hook output envelope.

[RULE]
Policy Kernel MUST be a pure function: same input → same output, no filesystem I/O.

[RULE]
Verdicts MUST be limited to: `allow`, `block`, `block_with_approval_request`, `force_continue`.

[RULE]
Hook scripts MUST read all stdin exactly once before parsing; partial reads are prohibited.

[RULE]
Hook scripts MUST emit **only** JSON on stdout. Any logging MUST go to stderr. Non-JSON stdout is a contract violation and MUST be detected in conformance tests.

[RULE]
Hooks MUST remain fast: heavy computation or blocking I/O MUST be moved to the policy kernel or cached; hook timeouts MUST be respected.

[RULE]
Hook contract MUST define an input_mutation field; it MAY only normalize deterministic inputs (path canonicalization, encoding, required headers). Semantic redirection MUST use block + approval.

[RULE]
SessionStart hooks are non-blocking. Exit 2 MUST only surface warnings and MUST NOT block the session.

[RULE]
Stop hook with `force_continue` MUST map to “keep working” semantics (Claude: decision=block, exit=0). Exit code 2 MUST NOT be used for stop continuation.

[RULE]
Stop enforcement MUST include an infinite-loop guard (lock or retry budget) to prevent perpetual continuation.

[RULE]
Hook Client MUST serialize journal appends for parallel hook executions and MUST NOT allow read-modify-write on journal.jsonl.

---

## 5. Journal Write Safety

[RULE]
`journal.jsonl` MUST be append-only via OS-level append semantics (O_APPEND). Read-before-write is forbidden.

[RULE]
On win32, journal writes MUST be serialized by a local queue or mutex.

[RULE]
Each journal line MUST be <4 KB. Oversized payloads MUST be externalized and referenced.

[RULE]
YAML state files (session/task-graph/sprint-status) MUST continue to use write-to-tmp then rename. This strategy MUST NOT be reused for journal appends.

[RULE]
Every journal line MUST end with `\n`; separator-injection based on prior file content is forbidden.

---

## 6. Skills Registry & Progressive Disclosure

[RULE]
Engine context files (CLAUDE.md, AGENTS.md, GEMINI.md) MUST only contain skill **stubs** (id, description, invocation policy, path).

[RULE]
Full skill instructions MUST live in generated skill files, loaded only when invoked.

[RULE]
Adapter compiler MUST perform three passes:
1) Stub pass (routing/context)
2) Skill pass (skill files)
3) Policy pass (engine enforcement artifacts)

[RULE]
Skill metadata MUST include activation and exclusion conditions; invocation_policy MUST be explicit | implicit | always.

[RULE]
Skills Registry MUST reference instruction bodies via file pointers and MUST include arguments schema and required_capabilities cross-ref.

[RULE]
Skills Registry MUST declare output contract references (template id + output level) per operation to enforce consistent user-facing output.

---

## 7. Execution Loop & Context Continuity

[RULE]
`session.yaml` tracks a single active run; `sprint-status.yaml` tracks cross-session story graph; `ledger.md` is append-only human-readable; `journal.jsonl` is the machine audit source of truth.

[RULE]
Autonomous loop budgets MUST be persisted in runtime state and enforced before any auto-advance. Crash recovery MUST restore a pre-write checkpoint, not a post-write snapshot.

[RULE]
Context continuity MUST externalize large artifacts into capsules and reference them from routing output; full skill bodies MUST NOT be injected into routing context files.

[RULE]
Review workflow MUST be two-stage (spec compliance → code quality). Policy kernel MUST enforce the order and record both verdicts in the journal.

[RULE]
TDD is a physical gate: production writes are blocked until a failing test is observed and verified (RED). Exceptions MUST require explicit approval.

---

## 8. Output Contract & Templates

[RULE]
V5 MUST standardize output templates for checkpoint, review, error, decision, progress, completion, and status outputs; these templates are the canonical user-facing contract.

[RULE]
Checkpoint output MUST be emitted after each node completion and include implementation summary, verification summary, self-audit status, progress, and next action.

[RULE]
Review output MUST include severity buckets and a final verdict.

[RULE]
Error output MUST include command, exit code, captured output, root cause, and next steps.

[RULE]
All outputs MUST be persisted to an output log and validated by a template validator; i18n and emoji/level conventions MUST be consistent across engines.

[RULE]
Skill start announcement MUST use a stable template (e.g., “🚀 Using [skill-name] to [purpose]”).

[RULE]
Output log MUST be append-only at `.ai/workflow/output.log` (or `.jsonl`) and contain template id + variables for deterministic replay.

[RULE]
Output validator MUST reject missing variables, unknown template ids, and nonconformant output levels.

[RULE]
Verification report templates MUST include build, typecheck, lint, tests (coverage), security scan, and diff summary sections.

---

## 9. Skills Change Detection & Refresh

[RULE]
Adapter compiler MUST emit `skills-manifest.json` (spec hash, generation time, artifact list). This file is the canonical signal for drift detection and hot reload.

[RULE]
Codex adapters MUST map `skills/changed` notifications to a registry reload + recompile.

[RULE]
Claude/Gemini adapters MUST provide a deterministic refresh path (e.g., explicit adapter `--write` or hook-triggered refresh) to avoid stale skill stubs.

---

## 10. Adapter Output Classes

| Class | Purpose | Examples |
|---|---|---|
| Routing artifacts | what to do next | `CLAUDE.md`, `AGENTS.md`, `GEMINI.md` |
| Skill artifacts | how to execute | `.claude/skills/*`, `.agents/skills/*` |
| Policy artifacts | what is blocked/allowed | `.claude/settings.json`, `.codex/config.toml`, `.gemini/policies/*.toml` |

[RULE]
Generated artifacts MUST carry generator metadata and drift detection. Seeded sections MUST be protected.

[RULE]
Each artifact MUST be classified as `generated` or `seeded`. The compiler MUST overwrite generated sections and MUST NOT mutate seeded sections.

[RULE]
Generated sections MUST be delimited with stable markers and must include generator + spec version metadata for drift checks.

---

## 11. Engine-Specific Enforcement Mapping

[RULE]
If an engine lacks a native hook surface, the adapter MUST map to the nearest stronger boundary (e.g., Codex sandbox + approval).

[RULE]
Gemini CLI MUST prefer native policy engine for access control; hooks remain for evidence capture and state sync.

[RULE]
Adapter compiler MUST maintain a canonical capability-gap mapping table as machine-readable data.

示例映射（规范基线，最终以映射表为准）：

| V5 Mechanism | Claude Code | Codex | Gemini CLI |
|---|---|---|---|
| Pre-write hard block | PreToolUse hook (exit 2) | sandbox tier + approval_policy | Admin-tier policy rule |
| TDD gate | PreToolUse hook → policy kernel | sandbox + approval_policy | Admin-tier TOML rule |
| Stop gate | Stop hook (force_continue) | approval_policy gate | AfterAgent hook retry |
| Approval request | hook output + systemMessage | request_permissions event | policy rule ask_user |

---

## 12. Migration Gates

[RULE]
`hook-contract.schema.yaml` MUST be frozen before P3-N1.

[RULE]
`skills.spec.yaml` MUST be frozen before P4-N1.

[RULE]
Existing hooks MAY directly read runtime state only until Hook Client is stable. After P3 exit gate, direct reads are prohibited.

[RULE]
Checkpoint A (Hook Client exists): direct reads are deprecated and must emit a warning. Checkpoint B (P3 exit): direct reads are prohibited and validator must fail.

---

## 13. Acceptance Criteria (V5)

[RULE]
No hook script reads runtime files directly after Hook Client stabilization.

[RULE]
Adapter compiler can generate routing/skill/policy artifacts from a single skills registry source without manual edits.

[RULE]
Journal append is concurrency-safe and passes multi-process fixtures without loss.

[RULE]
Stop hook `force_continue` semantics are verified by conformance tests for each engine.

[RULE]
Generated/seeded boundaries are enforced and drift detection ignores seeded regions.

[RULE]
Hook contract schema version mismatch blocks hook-client startup.

[RULE]
Hook stdout JSON-only rule is validated; any stdout pollution fails conformance.

[RULE]
Two-stage review ordering (spec → quality) is enforced and recorded.

[RULE]
TDD red gate blocks production writes unless explicit approval is recorded.

[RULE]
Policy mode mappings (plan/autoEdit/yolo) generate engine-appropriate artifacts and pass conformance.

[RULE]
Hook event matrix coverage (including BeforeToolSelection/AfterModel where available) is documented and adapter outputs include a capability-gap report.

[RULE]
Skills manifest drift is detectable; stale artifacts must fail conformance until regenerated.

---

## 14. Non-Goals

- V5 does not attempt to redesign personas or add new UI.
- V5 does not inline skill bodies into routing context files.
- V5 does not remove existing skill content before the Skills Registry is ready.

---

*End of Proposal*
