# Skill Triggering Tests

Validate whether `sy-*` skills are actually triggered from naive prompts.

## Purpose

- Prevent skill trigger regressions after editing frontmatter `description`.
- Verify "trigger-only description" quality with executable tests.
- Provide quick observability for "expected skill vs observed skill".

## Prerequisites

- Project root contains `.agents/skills/sy-*`.
- `runner` mode requires a supported CLI runner in PATH.
- `local` mode requires no external runner.

Supported runners:

- `claude`
- `codex`

You can switch runner with:

- `--runner <cmd>`
- or env: `SY_SKILL_TEST_RUNNER=codex`

## Usage

Run one case:

```bash
node tests/skill-triggering/run-test.cjs --skill sy-workflow --prompt tests/skill-triggering/prompts/sy-workflow.txt
```

Run one case in offline mode:

```bash
node tests/skill-triggering/run-test.cjs --mode local --skill sy-workflow --prompt tests/skill-triggering/prompts/sy-workflow.txt
```

Run all cases:

```bash
node tests/skill-triggering/run-all.cjs
```

Run all cases in offline mode:

```bash
node tests/skill-triggering/run-all.cjs --mode local
```

Run constraints pack (12 child skills + parent):

```bash
node tests/skill-triggering/run-all.cjs --mode local --cases tests/skill-triggering/cases.constraints.json
```

Run smoke pack for auto/runner regression:

```bash
node tests/skill-triggering/run-all.cjs --mode auto --cases tests/skill-triggering/cases.smoke.json
node tests/skill-triggering/run-all.cjs --mode runner --cases tests/skill-triggering/cases.smoke.json
```

Run with npm scripts:

```bash
npm run test:skills:core
npm run test:skills:core:auto
npm run test:skills:core:runner
npm run test:skills:smoke:auto
npm run test:skills:smoke:runner
npm run test:skills:constraints
```

`test:skills:core:auto` and `test:skills:core:runner` use `--timeout-ms 600000` to reduce false negatives on long runner responses.

Optional flags:

- `--max-turns <n>` (default: `3`)
- `--timeout-ms <n>` (default: `300000`)
- `--runner <cmd>` (default: `claude`)
- `--plugin-dir <path>` (optional, passed to runner when supported)
- `--mode <auto|runner|local>` (default: `auto`)
  - `auto`: prefer `runner`; fallback to `local` when runner command is unavailable
  - `runner`: must invoke external model runner; detect by skill events first, then semantic fallback when event is absent
  - `local`: deterministic offline assertions using `cases.json` keyword registry + frontmatter checks
- `--cases <path>` (default: `tests/skill-triggering/cases.json`)

Case packs:

- `tests/skill-triggering/cases.json`: core workflow skills
- `tests/skill-triggering/cases.auto.json`: full real-trigger regression pack for `--mode auto/runner`
- `tests/skill-triggering/cases.smoke.json`: lighter smoke pack for quick auto/runner checks
- `tests/skill-triggering/cases.constraints.json`: `sy-constraints` parent + child skills

Artifacts:

- runner log: `tests/skill-triggering/output/<timestamp>/<skill>/stream.json`
- runner detect summary: `tests/skill-triggering/output/<timestamp>/<skill>/runner-detect.json`
- local log: `tests/skill-triggering/output/<timestamp>/<skill>/local.json`
