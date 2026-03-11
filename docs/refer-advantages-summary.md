# Refer 文档可吸取的优点清单（V5 提炼）

> 目的：把 refer 目录的优势观点结构化沉淀，作为 V5 方案与后续实施的设计输入。

---

## 1) `skills-and-hooks-architecture-advisory.md`

- 跨引擎钩子生命周期与 IPC 契约的系统梳理，便于形成统一的 Hook 合约。
- 明确 “Hook 负责边界、Policy 负责决策”，利于抽离业务逻辑。
- Progressive Disclosure 得到多引擎共识（仅注入 stub，按需加载完整技能）。
- Gemini Policy Engine 的层级优先模型（admin/workspace/extension）可作为策略编译基线。
- 并发 Hook 场景下的 journal 竞争风险被明确提出，有利于提前治理。

---

## 2) `v4-architecture-update-proposal.md`

- 将 Skills Registry 纳入 Logical Specs，避免技能编译输入漂移。
- 明确 Hook 三层内部分工（脚本 / Hook Client / Kernel），利于职责隔离。
- 将编译产物拆分为 routing/skill/policy 三类，清晰可维护。

---

## 3) `v4-architecture-patch-risks.md`

- 用“风险驱动修正”方式，直接指向易错点（journal 并发写、迁移策略、生成边界）。
- 规范了 generated/seeded 边界，降低生成产物被手改导致漂移的风险。
- 引入 manifest + freeze_gate 的机器化约束，避免“方案未冻结即实现”。

---

## 4) `seeyue-workflow-Advanced-Agent-Engine-Architecture.md`

- 清晰定义运行时状态四层（session/sprint/ledger/journal），可审计、可恢复。
- 强调“State over Chat”，推动控制平面从 prompt 走向 runtime。
- 预算与恢复机制具备工程可落地性（loop budget、pre-write checkpoint）。
- 4 层约束 + persona isolation + TDD 物理门控的统一框架，提升治理强度。

---

## 5) `workflow-skills-system-design.md`

- 全量文件树与加载机制（L0/L1/L2）清晰，适合编译器自动化输出。
- 核心/辅助技能分层明确，覆盖研发全链路。
- 输出设计与验证流程成体系，为用户体验一致性打基础。

---

## 6) `workflow-skills-system-design-v3.md`

- 详细的操作级别契约（execute/test/verify 等）可直接迁移为 V5 “操作规范”输入。
- 4 级质量门与 execute 3-split 结构能减少“实现快但验证弱”的风险。
- 反模式门禁与回归验证思路可转化为 V5 的 conformance fixture 设计。

---

## 7) `workflow-skills-corrections.md`

- 基于官方文档的纠错清单，减少历史设计偏差。
- 关键 Hook 语义修正（stdin 一次性读取、SessionStart 非阻塞、Stop loop guard）。
- 缺失脚本补齐与 persona tool 权限规范，为后续迁移提供清晰修复路径。

---

## 8) `phase5-integration-summary.md`

- 输出设计 Phase 被系统化集成，包含模板库、输出持久化与验证路径。
- P0/P1/P2 实施路径明确，便于分批落地。

---

## 9) `output-templates-reference.md`

- 提供可直接落地的输出模板与变量定义，支持稳定机器解析。
- i18n 与颜色规范明确，利于跨引擎一致体验。

---

## 综合吸收方向（面向 V5）

- **控制平面化**：以 state store + policy kernel + adapter 为核心，减少 prompt 依赖。
- **工程可验证**：用 manifest/freeze_gate/fixture 驱动实现与回归。
- **输出可契约化**：模板化输出 + 校验脚本，统一“人机界面”。
