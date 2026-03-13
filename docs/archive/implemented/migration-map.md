# 从 VibeCast 抽离到 seeyue-workflows 的映射

## 已抽离资产

- `.agents/skills/sy-*` -> `.agents/skills/sy-*`
- `.claude/*` -> `.claude/*`
- `scripts/hooks/*` -> `scripts/hooks/*`
- `tests/hooks/*` -> `tests/hooks/*`
- `tests/skill-triggering/*` -> `tests/skill-triggering/*`
- `scripts/cleanup-skill-trigger-output.cjs` -> `scripts/cleanup-skill-trigger-output.cjs`

## 后续维护建议

1. 在独立仓库中维护 workflow 本体
2. 在独立仓库中先跑回归，再同步回业务仓库
3. 业务仓库只承载 workflow 的已验证版本
4. 如需发布版本，额外维护：
   - release note
   - version tag
   - changelog
