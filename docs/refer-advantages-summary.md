# Refer 文档可吸取的优点清单（V5 提炼）

> 目的：把 refer 目录的优势观点结构化沉淀，作为 V5 方案与后续实施的设计输入。

---

## 1) `skills-and-hooks-architecture-advisory.md`

- 跨引擎钩子生命周期与 IPC 契约的系统梳理，便于形成统一的 Hook 合约。
- 明确 “Hook 负责边界、Policy 负责决策”，利于抽离业务逻辑。
- Progressive Disclosure 得到多引擎共识（仅注入 stub，按需加载完整技能）。
- Gemini Policy Engine 的层级优先模型（admin/workspace/extension）可作为策略编译基线。
- 并发 Hook 场景下的 journal 竞争风险被明确提出，有利于提前治理。

落地状态（对照当前实现）：已落地 Hook 合约统一（`workflow/hook-contract.schema.yaml`）、Hook/Policy 分工（`scripts/runtime/hook-client.cjs` + `scripts/runtime/policy.cjs`）、技能 stub（`scripts/adapters/compile-adapter.cjs`）、journal append 安全（`scripts/runtime/store.cjs`）；未落地 Gemini admin/workspace/extension 层级优先模型（`workflow/policy.spec.yaml` 未建模）。

---

## 2) `v4-architecture-update-proposal.md`

- 将 Skills Registry 纳入 Logical Specs，避免技能编译输入漂移。
- 明确 Hook 三层内部分工（脚本 / Hook Client / Kernel），利于职责隔离。
- 将编译产物拆分为 routing/skill/policy 三类，清晰可维护。

落地状态（对照当前实现）：已落地（`workflow/skills.spec.yaml`、`scripts/hooks/*` + `scripts/runtime/hook-client.cjs` + `scripts/runtime/engine-kernel.cjs`、`scripts/adapters/compile-adapter.cjs` 的 routing/skills/policy pass）。

---

## 3) `v4-architecture-patch-risks.md`

- 用“风险驱动修正”方式，直接指向易错点（journal 并发写、迁移策略、生成边界）。
- 规范了 generated/seeded 边界，降低生成产物被手改导致漂移的风险。
- 引入 manifest + freeze_gate 的机器化约束，避免“方案未冻结即实现”。

落地状态（对照当前实现）：已落地 generated/seeded 边界（`scripts/adapters/adapter-utils.cjs` + `scripts/adapters/verify-adapter.cjs`）与 manifest/freeze gate（`workflow/validate-manifest.yaml` + `scripts/runtime/spec-validator.cjs`）；风险驱动修正属于方法论，保留为执行指导。

---

## 4) `seeyue-workflow-Advanced-Agent-Engine-Architecture.md`

- 清晰定义运行时状态四层（session/sprint/ledger/journal），可审计、可恢复。
- 强调“State over Chat”，推动控制平面从 prompt 走向 runtime。
- 预算与恢复机制具备工程可落地性（loop budget、pre-write checkpoint）。
- 4 层约束 + persona isolation + TDD 物理门控的统一框架，提升治理强度。

落地状态（对照当前实现）：已落地四层状态与预算/恢复（`.ai/workflow/session.yaml`、`sprint-status.yaml`、`ledger.md`、`journal.jsonl` + `scripts/runtime/checkpoints.cjs`）；部分落地 persona isolation 与 4 层约束硬性校验（约束已存在，但仍有路径依赖规范而非强制校验）。

---

## 5) `workflow-skills-system-design.md`

- 全量文件树与加载机制（L0/L1/L2）清晰，适合编译器自动化输出。
- 核心/辅助技能分层明确，覆盖研发全链路。
- 输出设计与验证流程成体系，为用户体验一致性打基础。

落地状态（对照当前实现）：部分落地技能加载与 stub 输出（`workflow/skills.spec.yaml` + `scripts/adapters/compile-adapter.cjs`）；输出模板/日志/校验已落地（`workflow/output-templates.spec.yaml` + `scripts/runtime/output-log.cjs` + `scripts/runtime/validate-output.cjs`）；核心/辅助分层仍属规范层面，缺少硬性校验。

---

## 6) `workflow-skills-system-design-v3.md`

- 详细的操作级别契约（execute/test/verify 等）可直接迁移为 V5 “操作规范”输入。
- 4 级质量门与 execute 3-split 结构能减少“实现快但验证弱”的风险。
- 反模式门禁与回归验证思路可转化为 V5 的 conformance fixture 设计。

落地状态（对照当前实现）：部分落地（TDD/Review/Verify 规则已在 `workflow/policy.spec.yaml` 与 `scripts/runtime/policy.cjs` 生效；`execute 3-split` 与完整操作级别契约尚未形成独立 spec）。

---

## 7) `workflow-skills-corrections.md`

- 基于官方文档的纠错清单，减少历史设计偏差。
- 关键 Hook 语义修正（stdin 一次性读取、SessionStart 非阻塞、Stop loop guard）。
- 缺失脚本补齐与 persona tool 权限规范，为后续迁移提供清晰修复路径。

落地状态（对照当前实现）：已落地 Hook 语义修正（`scripts/runtime/hook-client.cjs` + `scripts/hooks/*`）；persona tool 权限仍以规范为主，未形成全路径强制校验。

---

## 8) `phase5-integration-summary.md`

- 输出设计 Phase 被系统化集成，包含模板库、输出持久化与验证路径。
- P0/P1/P2 实施路径明确，便于分批落地。

落地状态（对照当前实现）：已落地输出模板 + output.log + 验证路径（`workflow/output-templates.spec.yaml`、`.ai/workflow/output.log`、`scripts/runtime/validate-output.cjs`）；P0/P1/P2 仍作为执行规划参考。

---

## 9) `output-templates-reference.md`

- 提供可直接落地的输出模板与变量定义，支持稳定机器解析。
- i18n 与颜色规范明确，利于跨引擎一致体验。

落地状态（对照当前实现）：已落地变量与模板注册（`workflow/output-templates.spec.yaml`）；示例模板正文与多引擎渲染仍属参考级说明（详见 `docs/refer/output-templates-reference.md` 的备注）。

---

## 综合吸收方向（面向 V5）

- **控制平面化**：以 state store + policy kernel + adapter 为核心，减少 prompt 依赖。
- **工程可验证**：用 manifest/freeze_gate/fixture 驱动实现与回归。
- **输出可契约化**：模板化输出 + 校验脚本，统一“人机界面”。
