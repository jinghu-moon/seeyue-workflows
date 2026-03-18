# Interaction Implementation Task List (P0-P2)

Status: draft  
Scope: Implement the interaction system defined by the approved P0-P2 documents and the machine contract  
Plan type: execution task list only (no implementation in this document)

---

## 1. Scope Gate

### In Scope

This plan covers implementation of the following source-of-truth set:

- `docs/hooks-mcp-interaction-refactor-plan.md`
- `docs/interaction-tui-architecture.md`
- `docs/sy-interact-cli-spec.md`
- `workflow/interaction.schema.yaml`
- `docs/interaction-runtime-integration.md`
- `docs/engine-interaction-mapping.md`
- `docs/hooks-interaction-bridge.md`
- `docs/mcp-interaction-bus.md`
- `docs/interaction-acceptance-criteria.md`

### Out of Scope

The following are explicitly out of scope for P0-P2:

- GUI dashboard implementation
- Full removal of all legacy `questions.jsonl` / `input_requests.jsonl` compatibility paths in the same phase as first rollout
- Rewriting all skill content around interaction-aware prompts
- Global engine UX redesign outside interaction-related paths
- Remote multi-user interaction orchestration

### Baseline Constraints

- Runtime remains the only decision authority.
- `sy-interact` remains presenter-only.
- Native engine capabilities remain primary; local presenter remains fallback or host-driven primary only where the engine lacks native interaction.
- `stdout` of `seeyue-mcp` remains clean JSON-RPC.
- New interaction storage must be durable, machine-readable, and resumable.

### Planning Note

All `verify.cmd`, `red_cmd`, and `green_cmd` entries below are planned implementation gates. Some commands reference tests that do not exist yet and are themselves part of the scope of the node/phase.

---

## 2. Baseline Snapshot

Existing implementation surface relevant to this plan:

- `seeyue-mcp/Cargo.toml`
- `seeyue-mcp/src/platform/terminal.rs`
- `seeyue-mcp/src/resources/workflow.rs`
- `seeyue-mcp/src/params/interactive.rs`
- `seeyue-mcp/src/tools/approval.rs`
- `seeyue-mcp/src/tools/ask_user.rs`
- `seeyue-mcp/src/tools/input_request.rs`
- `scripts/runtime/controller.cjs`
- `scripts/runtime/engine-kernel.cjs`
- `scripts/runtime/hook-client.cjs`
- `workflow/runtime.schema.yaml`
- `workflow/hooks.spec.yaml`
- `workflow/hook-contract.schema.yaml`

Known leverage points:

- Rust side already has `crossterm` and terminal initialization.
- MCP side already exposes workflow resources and interactive tool params.
- Runtime side already has blocker-first semantics (`recommended_next`, `restore_pending`, `approval_pending`) and durable state primitives.
- Hook side already has a thin-hook direction and a centralized hook client.

---

## 3. Phase P0 — Contract Freeze + Presenter Foundations

Source of Truth:

- `docs/interaction-tui-architecture.md`
- `docs/sy-interact-cli-spec.md`
- `workflow/interaction.schema.yaml`
- `docs/hooks-mcp-interaction-refactor-plan.md`

Phase Boundary:

entry_condition:
- approved interaction architecture exists in repo docs
- `workflow/interaction.schema.yaml` exists as draft machine contract
- no production implementation has started for `sy-interact`

exit_gate:
  cmd: >
    node "scripts/runtime/validate-specs.cjs" --spec "workflow/interaction.schema.yaml"
    && cargo build --manifest-path "seeyue-mcp/Cargo.toml" --bin sy-interact
    && cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_cli
  pass_signal: exit 0
  coverage_min: n/a

rollback_boundary:
  revert_nodes:
    - P0-N2
    - P0-N3
    - P0-N4
    - P0-N5
    - P0-N6
  restore_point: interaction schema validated, no runtime integration yet

### P0 Node Summary

| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| P0-N1 | spec validation | register interaction spec in validation flow | validate-specs | low | [] |
| P0-N2 | Rust bin scaffold | add `sy-interact` binary + module skeleton | cargo build | low | [P0-N1] |
| P0-N3 | DTO + IO | implement request/response IO and schema-aligned models | cargo test interaction_model | medium | [P0-N2] |
| P0-N4 | terminal probe | implement TTY/ANSI/raw/color-depth probing | cargo test interaction_terminal_probe | medium | [P0-N2] |
| P0-N5 | fallback renderers | implement `text_menu` / `plain_prompt` renderers | cargo test interaction_text_render | medium | [P0-N3,P0-N4] |
| P0-N6 | CLI contract | implement `render` + `probe-terminal` entrypoints and exit codes | cargo test interaction_cli | medium | [P0-N3,P0-N4,P0-N5] |

### P0 Detailed Nodes

#### P0-N1
- `id`: P0-N1
- `title`: Register interaction schema in validation and source-of-truth flow
- `target`:
  - `workflow/interaction.schema.yaml`
  - `workflow/validate-manifest.yaml`
  - `scripts/runtime/validate-specs.cjs`
  - `tests/runtime/run-spec-fixtures.cjs`
- `action`: Add `interaction.schema.yaml` to the repo’s formal validation/manifest flow so the contract is treated like existing workflow specs and blocks drift early.
- `why`: The interaction layer cannot remain “doc-only”; the schema must become enforceable before implementation consumers are added.
- `depends_on`: []
- `verify.cmd`: `node "scripts/runtime/validate-specs.cjs" --spec "workflow/interaction.schema.yaml"`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_required`: false

#### P0-N2
- `id`: P0-N2
- `title`: Add standalone `sy-interact` binary scaffold
- `target`:
  - `seeyue-mcp/Cargo.toml`
  - `seeyue-mcp/src/bin/sy-interact.rs`
  - `seeyue-mcp/src/interaction/mod.rs`
  - `seeyue-mcp/src/lib.rs`
- `action`: Introduce `sy-interact` as a separate Rust binary target inside the existing Rust crate, with an isolated `interaction` module tree and zero dependency on the MCP server main loop.
- `why`: This is the smallest implementation shape that preserves presenter isolation without introducing a second Rust workspace immediately.
- `depends_on`: [P0-N1]
- `verify.cmd`: `cargo build --manifest-path "seeyue-mcp/Cargo.toml" --bin sy-interact`
- `verify.pass_signal`: exit 0
- `risk_level`: low
- `tdd_required`: false

#### P0-N3
- `id`: P0-N3
- `title`: Implement schema-aligned request/response DTOs and file IO
- `target`:
  - `seeyue-mcp/src/interaction/model.rs`
  - `seeyue-mcp/src/interaction/io.rs`
  - `seeyue-mcp/src/error.rs`
  - `seeyue-mcp/tests/interaction_model.rs`
- `action`: Add serde-backed DTOs mirroring `workflow/interaction.schema.yaml`, plus request-file loading, response-file writing, and validation/normalization errors suitable for CLI use.
- `why`: All later UI and runtime work depends on stable, schema-shaped objects rather than ad-hoc maps.
- `depends_on`: [P0-N2]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_model`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_model_invalid_request`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_model_roundtrip`

#### P0-N4
- `id`: P0-N4
- `title`: Implement terminal capability probe and color-depth classification
- `target`:
  - `seeyue-mcp/src/platform/terminal.rs`
  - `seeyue-mcp/src/interaction/terminal_probe.rs`
  - `seeyue-mcp/tests/interaction_terminal_probe.rs`
- `action`: Reuse and extend the current terminal detection layer to classify `mono` / `ansi16` / `ansi256` / `rgb24`, raw-mode support, alternate-screen support, and recommended presentation mode.
- `why`: The presenter cannot safely choose TUI/text/plain behavior without a formal capability probe.
- `depends_on`: [P0-N2]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_terminal_probe`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_terminal_probe_unknown_caps`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_terminal_probe_profiles`

#### P0-N5
- `id`: P0-N5
- `title`: Implement fallback renderers before full TUI
- `target`:
  - `seeyue-mcp/src/interaction/render_text.rs`
  - `seeyue-mcp/src/interaction/render_plain.rs`
  - `seeyue-mcp/tests/interaction_text_render.rs`
- `action`: Implement `text_menu` and `plain_prompt` rendering paths, including option numbering, stable option ids, comment prompt behavior, and no-color fallback semantics.
- `why`: The project needs a reliable minimum interactive path before adding ratatui complexity.
- `depends_on`: [P0-N3, P0-N4]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_text_render`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_text_render_missing_comment`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_text_render_profiles`

#### P0-N6
- `id`: P0-N6
- `title`: Implement CLI contract and exit semantics
- `target`:
  - `seeyue-mcp/src/bin/sy-interact.rs`
  - `seeyue-mcp/src/interaction/cli.rs`
  - `seeyue-mcp/tests/interaction_cli.rs`
- `action`: Add `render` and `probe-terminal` subcommands, map internal failures to exit codes, preserve machine-readable stdout for probe mode, and ensure response files encode business results separately from process results.
- `why`: The CLI boundary is the stable contract used by host wrappers and future automation.
- `depends_on`: [P0-N3, P0-N4, P0-N5]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_cli`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_cli_invalid_request`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_cli_render_success`

---

## 4. Phase P1 — Local Presenter + Runtime Loop Integration

Source of Truth:

- `docs/interaction-runtime-integration.md`
- `docs/interaction-tui-architecture.md`
- `docs/sy-interact-cli-spec.md`
- `workflow/interaction.schema.yaml`

Phase Boundary:

entry_condition:
- P0 exit gate passes
- `sy-interact` can load request files and write response files
- no runtime consumer depends on prompt-only interaction fallback anymore

exit_gate:
  cmd: >
    node "tests/runtime/run-interaction-fixtures.cjs"
    && cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_tui
    && node "tests/runtime/run-checkpoint-fixtures.cjs" --case interaction-checkpoint
  pass_signal: exit 0
  coverage_min: n/a

rollback_boundary:
  revert_nodes:
    - P1-N1
    - P1-N2
    - P1-N3
    - P1-N4
    - P1-N5
    - P1-N6
    - P1-N7
  restore_point: sy-interact local binary works, runtime has no active interaction loop

### P1 Node Summary

| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| P1-N1 | interaction store | add durable request/response store and active index | runtime fixtures | medium | [P0-N6] |
| P1-N2 | runtime schema projection | extend session/runtime state with interaction block | validate-specs + runtime fixtures | medium | [P1-N1] |
| P1-N3 | request builders | project approval/question/input/restore into interaction requests | runtime fixtures | high | [P1-N1,P1-N2] |
| P1-N4 | host dispatcher | add host-side launcher that probes terminal and invokes `sy-interact` | integration fixture | medium | [P0-N6,P1-N1] |
| P1-N5 | full TUI renderer | implement ratatui UI, keymap, comment box, details panel | cargo test interaction_tui | high | [P0-N6,P1-N4] |
| P1-N6 | controller/router integration | wire blocker-first interaction handling into controller/router | controller fixtures | high | [P1-N3,P1-N4] |
| P1-N7 | journal/checkpoint/capsule | persist interaction events and recovery linkage | checkpoint fixtures | medium | [P1-N6] |
| P1-N8 | end-to-end local loop | add local happy-path + cancel-path fixtures | runtime fixtures | medium | [P1-N5,P1-N6,P1-N7] |

### P1 Detailed Nodes

#### P1-N1
- `id`: P1-N1
- `title`: Add durable interaction store and active index
- `target`:
  - `scripts/runtime/interaction-store.cjs`
  - `.ai/workflow/interactions/` layout logic
  - `tests/runtime/run-interaction-fixtures.cjs`
- `action`: Create runtime helpers for request/response storage, active interaction indexing, archive movement, and collision-safe file writes.
- `why`: A durable interaction loop requires the same state-first discipline as session/task/journal storage.
- `depends_on`: [P0-N6]
- `verify.cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case store-layout`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case store-missing-index`
- `green_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case store-layout`

#### P1-N2
- `id`: P1-N2
- `title`: Extend runtime schema and session projection for interaction state
- `target`:
  - `workflow/runtime.schema.yaml`
  - `scripts/runtime/state-repair.cjs`
  - `scripts/runtime/bootstrap-run.cjs`
  - `tests/runtime/run-state-repair-fixtures.cjs`
- `action`: Add an `interaction` block to runtime state, repair/bootstrap defaults, and schema validation to represent active interaction and pending count without duplicating full request payloads into session state.
- `why`: Runtime consumers need a stable projection without embedding the entire interaction object into `session.yaml`.
- `depends_on`: [P1-N1]
- `verify.cmd`: >
    node "scripts/runtime/validate-specs.cjs" --spec "workflow/runtime.schema.yaml"
    && node "tests/runtime/run-state-repair-fixtures.cjs" --case interaction-defaults
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-state-repair-fixtures.cjs" --case interaction-missing-defaults`
- `green_cmd`: `node "tests/runtime/run-state-repair-fixtures.cjs" --case interaction-defaults`

#### P1-N3
- `id`: P1-N3
- `title`: Build unified interaction request builders from existing runtime signals
- `target`:
  - `scripts/runtime/interaction-builder.cjs`
  - `scripts/runtime/controller.cjs`
  - `scripts/runtime/router.cjs`
  - `tests/runtime/run-interaction-fixtures.cjs`
- `action`: Convert approval, restore, question, input, and conflict signals into schema-aligned interaction requests, reusing existing `approval_pending`, `restore_pending`, `questions`, and `input_requests` pathways as migration inputs.
- `why`: This is the key DRY step that consolidates today’s fragmented human-input paths into one contract.
- `depends_on`: [P1-N1, P1-N2]
- `verify.cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case request-builder-all-kinds`
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case request-builder-approval-missing-scope`
- `green_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case request-builder-all-kinds`

#### P1-N4
- `id`: P1-N4
- `title`: Add host-side interaction dispatcher
- `target`:
  - `scripts/runtime/interaction-dispatch.cjs`
  - `scripts/runtime/controller.cjs`
  - `tests/runtime/run-interaction-fixtures.cjs`
- `action`: Add a host/wrapper script that discovers pending interactions, probes terminal capability, chooses `tui` / `text` / `plain`, invokes `sy-interact`, and returns response files to runtime.
- `why`: The presenter must be host-driven rather than hook-driven or prompt-driven.
- `depends_on`: [P0-N6, P1-N1]
- `verify.cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case dispatch-selects-mode`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case dispatch-no-probe-result`
- `green_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case dispatch-selects-mode`

#### P1-N5
- `id`: P1-N5
- `title`: Implement full ratatui-based menu presenter
- `target`:
  - `seeyue-mcp/Cargo.toml`
  - `seeyue-mcp/src/interaction/render_tui.rs`
  - `seeyue-mcp/src/interaction/state.rs`
  - `seeyue-mcp/src/interaction/theme.rs`
  - `seeyue-mcp/tests/interaction_tui.rs`
- `action`: Add the TUI renderer with focus management, single/multi-select support, comment input, details panel, status/help bar, theme tokens, and keyboard shortcuts described in the P0 docs.
- `why`: P0 proves the contract; P1 turns it into the intended keyboard-first UX.
- `depends_on`: [P0-N6, P1-N4]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_tui`
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_tui_keymap_failures`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_tui_submit_paths`

#### P1-N6
- `id`: P1-N6
- `title`: Integrate interaction loop into controller and router
- `target`:
  - `scripts/runtime/controller.cjs`
  - `scripts/runtime/router.cjs`
  - `scripts/runtime/transition-applier.cjs`
  - `tests/runtime/run-controller-fixtures.cjs`
  - `tests/router/run-router-fixtures.cjs`
- `action`: Make active interaction a blocker-first decision path, emit `resolve_interaction`-style `recommended_next`, and consume responses before resuming node/phase execution.
- `why`: Without controller/router integration, the presenter exists but does not control workflow flow.
- `depends_on`: [P1-N3, P1-N4]
- `verify.cmd`: >
    node "tests/runtime/run-controller-fixtures.cjs" --case interaction-blocks-loop
    && node "tests/router/run-router-fixtures.cjs" --case interaction-recommended-next
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-controller-fixtures.cjs" --case interaction-ignored-by-loop`
- `green_cmd`: `node "tests/runtime/run-controller-fixtures.cjs" --case interaction-blocks-loop`

#### P1-N7
- `id`: P1-N7
- `title`: Persist interaction events in journal, checkpoints, and handoff capsules
- `target`:
  - `scripts/runtime/checkpoints.cjs`
  - `scripts/runtime/runtime-snapshot.cjs`
  - `scripts/runtime/report-builder.cjs`
  - `tests/runtime/run-checkpoint-fixtures.cjs`
- `action`: Add interaction event emission, active interaction checkpoint linkage, and capsule/handoff summaries to preserve blocked user decisions across resume and compaction paths.
- `why`: Interaction state must survive interruption just like node/phase state.
- `depends_on`: [P1-N6]
- `verify.cmd`: `node "tests/runtime/run-checkpoint-fixtures.cjs" --case interaction-checkpoint`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-checkpoint-fixtures.cjs" --case interaction-missing-capsule`
- `green_cmd`: `node "tests/runtime/run-checkpoint-fixtures.cjs" --case interaction-checkpoint`

#### P1-N8
- `id`: P1-N8
- `title`: Add end-to-end local interaction fixtures
- `target`:
  - `tests/runtime/run-interaction-fixtures.cjs`
  - `seeyue-mcp/tests/interaction_cli.rs`
- `action`: Add happy path, cancel path, timeout path, and non-TTY fallback fixtures spanning runtime request creation, host dispatch, presenter response, and runtime resume.
- `why`: This establishes the first complete local loop before hook/MCP/adapter integrations.
- `depends_on`: [P1-N5, P1-N6, P1-N7]
- `verify.cmd`: `node "tests/runtime/run-interaction-fixtures.cjs"`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case local-loop-incomplete`
- `green_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs"`

---

## 5. Phase P2 — Hooks + MCP + Adapter Integration and Acceptance

Source of Truth:

- `docs/hooks-interaction-bridge.md`
- `docs/mcp-interaction-bus.md`
- `docs/engine-interaction-mapping.md`
- `docs/interaction-acceptance-criteria.md`
- `workflow/interaction.schema.yaml`

Phase Boundary:

entry_condition:
- P1 exit gate passes
- local presenter loop is stable
- runtime interaction store and router integration are in place

exit_gate:
  cmd: >
    node "tests/hooks/run-hook-interaction-fixtures.cjs"
    && cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp
    && node "tests/e2e/run-engine-conformance.cjs" --case interaction
  pass_signal: exit 0
  coverage_min: n/a

rollback_boundary:
  revert_nodes:
    - P2-N1
    - P2-N2
    - P2-N3
    - P2-N4
    - P2-N5
    - P2-N6
    - P2-N7
  restore_point: local runtime interaction loop works; hook/MCP/adapter integration absent

### P2 Node Summary

| ID | Target | Action | Verify | Risk | Depends |
|---|---|---|---|---|---|
| P2-N1 | hook spec | add `failure_mode` and validation updates | hook spec tests | medium | [P1-N8] |
| P2-N2 | hook-client bridge | add interaction-aware decision envelope and runtime handoff | hook fixtures | high | [P2-N1] |
| P2-N3 | legacy interactive tool convergence | unify approval/ask/input into interaction projection layer | cargo/node tests | high | [P1-N3,P2-N2] |
| P2-N4 | MCP resources/tools | expose active interactions and structured outputs | cargo test interaction_mcp | high | [P1-N8,P2-N3] |
| P2-N5 | MCP input strategy | add elicitation-native-first / presenter-fallback orchestration | cargo test interaction_mcp_client | high | [P2-N4] |
| P2-N6 | adapter/compiler mapping | implement capability-gap output and engine-specific interaction routing | adapter tests | medium | [P2-N4,P2-N5] |
| P2-N7 | acceptance harness | implement P2 acceptance matrix and conformance tests | e2e conformance | medium | [P2-N1,P2-N4,P2-N6] |

### P2 Detailed Nodes

#### P2-N1
- `id`: P2-N1
- `title`: Add `failure_mode` to hook spec and validation
- `target`:
  - `workflow/hooks.spec.yaml`
  - `scripts/runtime/validate-specs.cjs`
  - `tests/hooks/run-hook-template-fixtures.cjs`
- `action`: Extend the hook spec with `hard_gate` / `advisory` / `telemetry`, validate the field, and freeze the intended semantics for interaction-sensitive events.
- `why`: Interaction-aware blocking cannot be made safe while hook failures still collapse into implicit behavior.
- `depends_on`: [P1-N8]
- `verify.cmd`: `node "tests/hooks/run-hook-template-fixtures.cjs" --case hooks-failure-mode`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/hooks/run-hook-template-fixtures.cjs" --case hooks-missing-failure-mode`
- `green_cmd`: `node "tests/hooks/run-hook-template-fixtures.cjs" --case hooks-failure-mode`

#### P2-N2
- `id`: P2-N2
- `title`: Extend hook client with interaction-aware decision envelope
- `target`:
  - `scripts/runtime/hook-client.cjs`
  - `scripts/runtime/engine-kernel.cjs`
  - `tests/hooks/run-hook-interaction-fixtures.cjs`
- `action`: Add `interaction_required`, `interaction_kind`, `blocking_kind`, `reason_code`, `risk_level`, and `scope` to the hook decision envelope, then hand off interaction creation to runtime instead of hooks.
- `why`: Hooks must become a clean bridge into the unified interaction model rather than a parallel control path.
- `depends_on`: [P2-N1]
- `verify.cmd`: `node "tests/hooks/run-hook-interaction-fixtures.cjs" --case decision-envelope`
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `node "tests/hooks/run-hook-interaction-fixtures.cjs" --case missing-interaction-envelope`
- `green_cmd`: `node "tests/hooks/run-hook-interaction-fixtures.cjs" --case decision-envelope`

#### P2-N3
- `id`: P2-N3
- `title`: Converge legacy approval/ask/input pathways onto interaction projection
- `target`:
  - `seeyue-mcp/src/tools/approval.rs`
  - `seeyue-mcp/src/tools/ask_user.rs`
  - `seeyue-mcp/src/tools/input_request.rs`
  - `seeyue-mcp/src/params/interactive.rs`
  - `scripts/runtime/interaction-builder.cjs`
  - `seeyue-mcp/tests/interaction_migration.rs`
- `action`: Keep backward-compatible interactive tools, but project them into the unified interaction store and schema so approvals, questions, and inputs stop diverging semantically.
- `why`: The repo already has useful primitives; this node avoids duplicate systems and reduces migration risk.
- `depends_on`: [P1-N3, P2-N2]
- `verify.cmd`: >
    cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_migration
    && node "tests/runtime/run-interaction-fixtures.cjs" --case legacy-to-interaction-projection
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case legacy-projection-missing-comment-mode`
- `green_cmd`: `node "tests/runtime/run-interaction-fixtures.cjs" --case legacy-to-interaction-projection`

#### P2-N4
- `id`: P2-N4
- `title`: Expose interaction resources and structured MCP outputs
- `target`:
  - `seeyue-mcp/src/resources/workflow.rs`
  - `seeyue-mcp/src/server/mod.rs`
  - `seeyue-mcp/src/server/tools_ia.rs`
  - `seeyue-mcp/tests/interaction_mcp.rs`
- `action`: Add resources for active interaction/index/item, enrich tool outputs with `structuredContent`, and expose interaction-oriented read/write surfaces without introducing local UI into the MCP server.
- `why`: Remote/native interactive clients need the same interaction data model as the local presenter.
- `depends_on`: [P1-N8, P2-N3]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp`
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp_missing_resource`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp`

#### P2-N5
- `id`: P2-N5
- `title`: Add MCP input-strategy orchestration (`elicitation` first, presenter fallback)
- `target`:
  - `seeyue-mcp/src/tools/` interaction-specific orchestration modules
  - `seeyue-mcp/src/workflow/state.rs`
  - `seeyue-mcp/tests/interaction_mcp_client.rs`
  - `scripts/runtime/interaction-dispatch.cjs`
- `action`: Add capability-aware logic so MCP clients that support `elicitation` can resolve interactions natively, while unsupported/local-only paths still route to `sy-interact` or text fallback.
- `why`: This node makes the interaction model engine-neutral without forcing every client through the same UX surface.
- `depends_on`: [P2-N4]
- `verify.cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp_client`
- `verify.pass_signal`: exit 0
- `risk_level`: high
- `tdd_required`: true
- `red_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp_client_no_fallback`
- `green_cmd`: `cargo test --manifest-path "seeyue-mcp/Cargo.toml" interaction_mcp_client_elicitation_then_fallback`

#### P2-N6
- `id`: P2-N6
- `title`: Implement engine adapter/compiler interaction mapping and capability-gap outputs
- `target`:
  - `scripts/adapters/compile-adapter.cjs`
  - `scripts/adapters/verify-adapter.cjs`
  - adapter-generated artifacts for Claude/Codex/Gemini
  - `tests/adapters/run-adapter-snapshots.cjs`
- `action`: Teach the adapter/compiler layer how to map interaction semantics to engine-native paths, emit capability-gap reports, and represent `sy-interact` as a local-presenter fallback rather than an engine-embedded feature.
- `why`: Cross-engine consistency must live in adapters, not in prompt improvisation.
- `depends_on`: [P2-N4, P2-N5]
- `verify.cmd`: `node "tests/adapters/run-adapter-snapshots.cjs" --suite interaction`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/adapters/run-adapter-snapshots.cjs" --case interaction-capability-gap-missing`
- `green_cmd`: `node "tests/adapters/run-adapter-snapshots.cjs" --suite interaction`

#### P2-N7
- `id`: P2-N7
- `title`: Implement acceptance matrix and engine conformance gates
- `target`:
  - `tests/hooks/run-hook-interaction-fixtures.cjs`
  - `tests/runtime/run-interaction-fixtures.cjs`
  - `tests/e2e/run-engine-conformance.cjs`
  - `docs/interaction-acceptance-criteria.md` (sync if acceptance expands)
- `action`: Turn the acceptance document into executable fixture suites covering keyboard fallback, runtime loop, hook bridge, MCP resources/tools, adapter mapping, and engine conformance.
- `why`: P2 is only complete when the interaction model is auditable and regression-resistant across surfaces.
- `depends_on`: [P2-N1, P2-N4, P2-N6]
- `verify.cmd`: `node "tests/e2e/run-engine-conformance.cjs" --case interaction`
- `verify.pass_signal`: exit 0
- `risk_level`: medium
- `tdd_required`: true
- `red_cmd`: `node "tests/e2e/run-engine-conformance.cjs" --case interaction-failing-baseline`
- `green_cmd`: `node "tests/e2e/run-engine-conformance.cjs" --case interaction`

---

## 6. Parallelization Guidance

Parallel groups are allowed only where file overlap and state risk stay controlled.

Recommended candidates:

- `parallel_group: p0_core`
  - `P0-N3` and `P0-N4` MAY run in parallel after `P0-N2`
- `parallel_group: p1_local`
  - `P1-N4` and `P1-N5` MAY partially overlap after `P0-N6`, but integration merge should wait for both
- `parallel_group: p2_remote`
  - `P2-N4` and the early scaffolding of `P2-N6` MAY overlap once interaction resources are shaped

Explicitly non-parallel:

- `P1-N6` MUST wait for `P1-N3` and `P1-N4`
- `P2-N2` MUST wait for `P2-N1`
- `P2-N7` MUST remain last in phase

---

## 7. Risk Register

| Risk | Phase | Level | Mitigation |
|---|---|---|---|
| Presenter grows into a policy engine | P0-P2 | high | Keep runtime as sole decision authority; keep request/response files thin |
| `sy-interact` lifecycle contaminates MCP stdio | P0-P2 | high | Keep `sy-interact` as separate bin, never inside `seeyue-mcp` main loop |
| Runtime state forks between legacy approvals/questions/inputs and interaction store | P1-P2 | high | Use projection/migration layer before any hard cutover |
| Hook failures still silently degrade safety | P2 | high | Add `failure_mode` and contract tests before interaction bridge rollout |
| Cross-engine behavior drifts | P2 | medium | Add capability-gap report and adapter conformance fixtures |
| Terminal capability detection differs across Windows hosts | P0-P1 | medium | Add probe + fallback hierarchy + no-color path |

---

## 8. Ready-to-Execute Marker

This plan is execution-ready once you approve the phase order and file placement choices below:

- `sy-interact` implemented as a new bin under `seeyue-mcp`, not a separate workspace crate
- interaction durability rooted under `.ai/workflow/interactions/`
- local presenter is host-driven
- legacy approval/question/input tools are converged via projection, not hard-removed in first rollout

Awaiting approval. Reply `执行 P0` to start implementation from `P0-N1`.
