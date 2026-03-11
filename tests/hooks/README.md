# Hook Smoke Tests

Quick deterministic checks for `scripts/hooks/*` guard behavior.

## Usage

```bash
node tests/hooks/sy-hooks-smoke.cjs
```

or

```bash
npm run test:hooks:smoke
```

## Coverage

- `sy-pretool-bash.cjs`: allow safe command + block force push
- `sy-pretool-write.cjs`: allow env reference + block hardcoded token + TDD red gate
- `sy-pretool-write-session.cjs`: block invalid `current_phase` in canonical `session.yaml` writes (legacy `session.md` 兼容)
- `sy-posttool-bash-verify.cjs`: capture verify evidence into staging report
- `sy-stop.cjs`: block incomplete execute checkpoint + allow review stop with fresh report
