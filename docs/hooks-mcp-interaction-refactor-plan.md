# seeyue-workflows Hooks + MCP 重构方案（交互设计优先）

状态：draft  
受众：workflow maintainers、runtime authors、MCP authors、adapter compiler owners  
范围：Hooks + MCP；Skills 仅讨论与交互面直接相关的部分  
基线：`docs/architecture-v4.md`、`docs/architecture-v5-proposal.md`  
主要参考：`refer/skills-and-hooks-architecture-advisory.md`、`refer/agent-source-code/claude-code-main/`、`refer/agent-source-code/codex-main/`、`refer/agent-source-code/gemini-cli-main/`、`refer/mcp-source/modelcontextprotocol-main/`

---

## 1. 问题定义

基于对仓库本体、`refer/` 参考源码、MCP 官方资料与三类主流 agent CLI 的对照分析，可以确认：

- 本项目的本质不是业务应用，而是一个面向多引擎的 workflow control plane。
- 真正的机器真相源是 `workflow/*.yaml`，而不是生成产物 `AGENTS.md`、`CLAUDE.md`、`GEMINI.md`。
- 当前最需要补强的不是“再增加一些 hook 脚本”或“再堆更多 MCP tools”，而是统一的交互模型。
- 目前 Hooks、MCP、Skills 三者的职责边界已初步成型，但用户交互、agent 交互、运行时交互仍然分散在多个实现里，缺少稳定的一致语义。

这会直接带来五类问题：

1. **阻塞语义不统一**：approval、restore、question、checkpoint、stop 各自有实现，但缺少统一交互对象。
2. **跨引擎体验不一致**：Claude、Codex、Gemini 原生能力差异大，当前更像“功能对齐”，还不是“交互对齐”。
3. **Hook 负担过重**：部分策略判断仍容易滑入 hook 层，导致可测试性、可解释性、可迁移性变差。
4. **MCP 还偏工具层**：已经具备资源与 prompt 能力，但还没有真正升级为“状态 + 交互总线”。
5. **用户心智成本偏高**：用户知道系统会拦截，但不总能立刻知道“为什么被拦、下一步该做什么、如何恢复、如何继续”。

因此，下一轮重构必须把重点放在**交互设计**：先把交互契约统一，再让 Hooks、MCP、Skills 各做自己最擅长的部分。

---

## 2. 核心判断

### 2.1 三层分工不变，但职责要进一步收口

- **Skills**：负责流程与认知编排，决定“该怎么思考、按什么步骤执行”。
- **Hooks**：负责执行边界，决定“这个动作此刻能不能发生、是否需要拦截、是否需要证据落盘”。
- **MCP**：负责能力与状态总线，决定“哪些上下文可被发现、哪些交互可被结构化暴露、哪些用户输入需要回流运行时”。

### 2.2 本阶段优先级

本阶段优先级应该明确为：

1. 先把 **Hooks 变薄**。
2. 再把 **MCP 从工具服务器升级为交互总线**。
3. 最后再把 **Skills 系统化**，并统一编译为多引擎产物。

原因很简单：

- 如果没有稳定的交互契约，Skills 再系统化也会被不同引擎的交互差异不断冲垮。
- 如果 Hooks 继续承担策略与交互双重职责，MCP 就很难成为单一状态总线。
- 如果 MCP 只做 tools，不做交互对象与订阅机制，系统就无法稳定支持 approval、restore、question、checkpoint、handoff 等关键流程。

### 2.3 交互优先于功能堆叠

对本项目来说，真正的“高级能力”不是再多几个工具，而是：

- 被拦截时，用户能立即看懂原因与影响范围；
- 需要确认时，用户能用最短路径完成确认；
- 需要恢复时，系统能明确告诉用户恢复原因、恢复入口与恢复后下一步；
- 上下文压缩或会话切换后，agent 能稳定恢复，不依赖聊天历史猜测。

---

## 3. 设计目标

本方案的目标不是做一个“更复杂的运行时”，而是做一个**交互一致、状态清晰、易于跨引擎映射**的控制平面。

### 3.1 用户侧目标

- 用户始终能看到当前阻塞项、原因、风险级别、影响范围、推荐下一步。
- 所有高风险确认都使用统一、短促、可执行的文案。
- 所有 restore / resume / continue / approve 都有清晰入口，而不是依赖自由文本协商。
- 在长会话、压缩上下文、引擎切换后，交互语义保持稳定。

### 3.2 Agent 侧目标

- agent 面对的是统一的交互对象，而不是各引擎私有行为拼贴。
- `recommended_next`、`restore_reason`、`approval_pending` 等关键状态由 runtime 决定，而不是聊天临场生成。
- hook 只做“边界判断 + 翻译 + 证据捕获”，不再承载复杂策略。

### 3.3 系统侧目标

- MCP 成为结构化交互总线，支持资源发现、订阅、prompts、elicitation、structured tool output。
- 运行时对交互状态有明确持久化模型，可审计、可恢复、可回放。
- 各引擎优先使用原生能力，adapter 负责降级映射，而不是做伪统一。

---

## 4. 非目标

本方案明确不做以下事情：

- 不在本阶段重写所有 Skills 内容。
- 不把 Codex 强行模拟成 Claude 式 hooks 系统。
- 不把复杂策略判断重新塞回 hook 脚本。
- 不在本阶段引入新的 GUI 或 dashboard 前端。
- 不以“更多自动化”为目标牺牲 human-in-the-loop 的清晰性。

---

## 5. 交互设计原则

### 5.1 状态优先

所有关键交互必须以运行时状态为准，不允许由自由文本临时拼装核心决策。

具体要求：

- `recommended_next` 必须来自 router/runtime。
- `restore_reason` 必须来自 recovery/checkpoint 子系统。
- `approval_pending` 必须来自 approval 状态对象。
- prompts 只负责引导，不负责篡改状态真相。

### 5.2 阻塞优先

系统必须优先暴露 blocker，而不是继续输出长篇建议。

优先级建议：

1. `restore_pending`
2. `approval_pending`
3. `question_pending`
4. `input_required`
5. 普通 `recommended_next`

### 5.3 明示优于暗示

所有需要用户动作的场景，都应该有结构化请求对象，而不是靠“你现在最好……”“建议你先……”这种软提示。

### 5.4 中断可恢复

任何会话结束、上下文压缩、checkpoint、stop、handoff 都必须留下可恢复对象，而不是只留下自然语言总结。

### 5.5 原生能力优先

- Claude：优先使用 managed hooks、managed permission rules。
- Codex：优先使用 `AGENTS.md`、`approval_policy`、`sandbox_mode`、`mcp_servers`。
- Gemini：优先使用 `BeforeToolSelection`、policy engine、checkpointing、trust 配置。

### 5.6 Progressive disclosure

- `AGENTS.md` / `CLAUDE.md` / `GEMINI.md` 只做路由入口，不做百科全书。
- Skills 通过 registry + prompt 获取按需展开。
- MCP resources/prompts 只暴露当前必要上下文，不做冗余灌输。

---

## 6. Component Map

| 组件 | 职责 | 输入/输出 | 约束 |
|---|---|---|---|
| Runtime Kernel | 唯一决策中心，输出 blocker、verdict、recommended_next | 输入：session/task-graph/policy/hook event；输出：decision envelope | 不依赖自由文本，不直接面向引擎差异 |
| Hook Client | 统一 hook IPC、快照、错误处理、journal 写入 | 输入：引擎 hook event；输出：标准化 decision request/result | 必须薄，不承载业务策略 |
| Thin Hooks | 物理边界拦截与引擎格式翻译 | 输入：原生 hook payload；输出：hook client 调用 | stdout 仅 JSON；快；无复杂状态读取 |
| MCP Server | 资源、prompts、tools、elicitation、订阅总线 | 输入：runtime store；输出：结构化交互面 | 要求 schema 明确、可订阅、可扩展 |
| Adapter Compiler | 将真相源编译到 Claude/Codex/Gemini 产物 | 输入：workflow specs；输出：engine artifacts | 吸收引擎差异，禁止伪统一 |
| Durable Runtime Store | 保存 session、journal、approvals、questions、checkpoints | 输入：kernel / tools / hooks；输出：可审计状态 | append-only、可恢复、机器可读 |
| Skills Registry | 维护 Skills 元数据与 prompt 入口 | 输入：`workflow/skills.spec.yaml`；输出：prompt registry / skill artifacts | 必须 progressive disclosure |
| Trust Store | 记录 hook / MCP server / tool 的信任状态 | 输入：用户确认或系统策略；输出：trust verdict | 必须可撤销、可审计、可见 |

---

## 7. Tech Stack Decisions

| 领域 | 选型 | 备选 | 选择理由 |
|---|---|---|---|
| Hook 策略承载 | Runtime Kernel | hook 脚本内判断 | 降低漂移，便于测试与跨引擎复用 |
| Hook 失败语义 | `failure_mode` 分级 | 全部 fail-open / 全部 fail-close | 不同交互风险需要不同失败策略 |
| 交互真相源 | 新增 `workflow/interaction.schema.yaml` | 仅靠零散 JSONL 文件 | 让 approval/restore/question/checkpoint 有统一机器契约 |
| 用户补充输入 | MCP `elicitation` 优先 | 纯自然语言追问 | 可结构化、可绑定原始请求、可审计 |
| 状态读取 | 资源 + dashboard 聚合 | 每次散读多个文件 | 降低 agent 心智负担与调用成本 |
| 状态刷新 | `listChanged` + `subscribe` | 轮询 | 更接近实时交互总线 |
| 技能入口 | `skills.spec.yaml` + MCP prompts | 在 AGENTS 中内联技能全文 | 保持 progressive disclosure，避免上下文爆炸 |
| Codex 适配 | config-first | hook bridge 模拟 | Codex 的原生边界本来就不在 Claude 式 hooks |
| Gemini 适配 | policy + `BeforeToolSelection` | 全部转 shell hook | Gemini 原生支持更强，没必要降级 |

---

## 8. 交互对象模型（建议新增机器真相源）

建议新增：`workflow/interaction.schema.yaml`

这个 schema 不替代 `runtime.schema.yaml`，而是补足“交互对象”的统一定义，解决目前 approval、restore、question、checkpoint、trust、handoff 等语义分散的问题。

### 8.1 为什么需要单独的 interaction schema

当前仓库已经有：

- `workflow/runtime.schema.yaml`
- `workflow/hooks.spec.yaml`
- `workflow/policy.spec.yaml`
- `workflow/router.spec.yaml`
- `workflow/skills.spec.yaml`

但还缺一个关键层：**交互对象层**。

缺少这一层会导致：

- hook 返回值与 MCP tool 输出之间没有统一对象；
- approval / restore / question / input request 的字段语义容易逐步分叉；
- adapter 很难做到跨引擎一致映射；
- dashboard、prompt、日志、恢复逻辑都容易形成各自的小协议。

### 8.2 建议的核心对象

建议最少定义以下对象：

1. `interaction_result`
2. `approval_request`
3. `restore_request`
4. `question_request`
5. `input_request`
6. `checkpoint_notice`
7. `handoff_capsule`
8. `trust_record`

### 8.3 建议的最小公共字段

所有交互对象建议共享以下元字段：

- `id`
- `type`
- `status`
- `origin`
- `originating_request_id`
- `created_at`
- `updated_at`
- `reason_code`
- `message`
- `risk_level`
- `scope`
- `recommended_next`

### 8.4 `interaction_result` 建议字段

```yaml
interaction_result:
  verdict: allow | block | block_with_approval_request | force_continue | ask_question | request_input
  blocker_kind: none | approval | restore | question | input | policy | trust
  reason_code: string
  message: string
  recommended_next: []
  writes_blocked: boolean
  requires_user_action: boolean
  interaction_ref: string?
```

### 8.5 `approval_request` 建议字段

```yaml
approval_request:
  approval_id: string
  action: string
  target: string
  risk_level: low | medium | high | critical
  impact_scope:
    files: []
    commands: []
    services: []
  approval_mode: explicit | notify_only
  grant_scope: once | node | phase | session
  reason_code: string
  user_message: string
  status: pending | approved | denied | expired
  originating_request_id: string
```

### 8.6 `restore_request` 建议字段

```yaml
restore_request:
  restore_id: string
  restore_reason: string
  checkpoint_id: string?
  restore_mode: automatic | manual
  blocking: true
  user_message: string
  required_action: string
  status: pending | restored | failed | dismissed
```

### 8.7 `handoff_capsule` 建议字段

```yaml
handoff_capsule:
  capsule_id: string
  run_id: string
  phase_id: string
  node_id: string
  blockers: []
  recommended_next: []
  evidence_refs: []
  open_questions: []
  created_at: string
```

---

## 9. 目标交互架构

```text
User / Client
  ↓
Engine Native Surface
  - Claude: hooks + permission rules
  - Codex: AGENTS + approval_policy + sandbox_mode + mcp_servers
  - Gemini: BeforeToolSelection + policy + checkpointing + trust
  ↓
Thin Hooks / Native Policy Bridge
  ↓
Hook Client / Adapter Bridge
  ↓
Runtime Kernel
  ↓
MCP Interaction Bus
  - resources
  - prompts
  - tools
  - elicitation
  - subscribe/listChanged
  ↓
.ai/workflow Durable Store
```

关键原则：

- **Runtime Kernel 是唯一决策中心**。
- **Hooks 只做边界，不做主策略**。
- **MCP 负责暴露交互对象，不替代 runtime 决策**。
- **Adapter 负责吸收引擎差异，不创造第二份真相源**。

---

## 10. 关键交互流

### 10.1 安全路径：无阻塞执行

目标：让“没有风险、没有 blocker 的动作”尽可能顺滑。

流程：

1. 用户提出请求。
2. agent 通过 `workflow://dashboard` 读取当前聚合状态。
3. adapter 选择工具或操作。
4. 原生 hook / policy surface 触发。
5. Hook Client 将请求送入 Runtime Kernel。
6. Runtime Kernel 返回 `interaction_result.verdict = allow`。
7. 工具执行。
8. PostToolUse 只写证据与 journal。
9. Stop / AfterAgent 刷新 `recommended_next` 与 capsule。

设计要点：

- 优先读聚合 dashboard，而不是每次散读多个资源。
- 成功路径不要被复杂交互打断。
- 证据收集放在 post 阶段，避免污染 pre 阶段判断。

### 10.2 风险路径：需要审批

目标：让高风险操作“能解释、可控制、可审计”。

流程：

1. Hook/Policy 对命令或写入做风险分类。
2. Kernel 产出 `block_with_approval_request`。
3. adapter 将其映射到引擎原生审批机制；若无原生机制，则通过 MCP/JSONL fallback 暴露给用户。
4. 用户看到统一结构：操作类型、影响范围、风险、授权范围。
5. 用户批准或拒绝。
6. approval 状态写入 durable store。
7. `recommended_next` 重新计算。

统一文案建议：

```text
⚠️ 危险操作检测！
操作类型：<action>
影响范围：<scope>
风险评估：<risk>

请确认是否继续？[需要明确的“是”、“确认”、“继续”]
```

设计要点：

- 高风险确认一定要短、明确、可执行。
- 不允许“长篇解释淹没关键按钮”。
- `grant_scope` 必须显式可见，避免误授权。

### 10.3 信息缺失路径：question / input request

目标：把“问用户”从自由文本提升为结构化交互。

流程：

1. Kernel 发现缺少关键输入，但不属于高风险审批。
2. 产出 `ask_question` 或 `request_input`。
3. 若客户端支持 MCP `elicitation`，则优先走该路径。
4. 若客户端不支持，则落到 `questions.jsonl` / `input_requests.jsonl`，并通过 `workflow://questions` / `workflow://inputs` 暴露。
5. 用户回答后，状态回写运行时。
6. runtime 重新生成 `recommended_next`。

设计要点：

- question 应短而有限，优先结构化选项。
- 敏感输入必须走 MCP 规范允许的安全输入模式。
- 输入请求必须绑定 `originating_request_id`，防止脱链。

### 10.4 恢复路径：restore_pending

目标：所有恢复场景都先恢复，再继续做新事。

流程：

1. checkpoint / handoff / restore 逻辑检测到恢复未完成。
2. session 进入 `restore_pending`。
3. runtime 输出 `restore_request` 与 blocker-first `recommended_next`。
4. adapter 必须优先呈现恢复请求，而不是继续让 agent推进新工作。
5. 恢复完成后，清除 `restore_pending`，重新进入主流程。

设计要点：

- `restore_reason` 必须具体，不允许模糊描述。
- 如果需要人工恢复，消息必须是短促、动作化的中文。
- 未恢复时，禁止新的写操作与阶段推进。

### 10.5 压缩与交接路径：PreCompact / handoff

目标：让长会话压缩与多轮交接不丢关键上下文。

流程：

1. 触发 PreCompact / BeforeCompact / 会话收口。
2. 系统生成 `handoff_capsule` 与 checkpoint。
3. capsule 至少包含：当前 phase/node、blockers、recommended_next、证据引用、未回答问题。
4. 新轮次优先用 capsule 恢复，而不是依赖聊天摘要重建状态。
5. 如果 capsule 不完整，直接进入 `restore_pending`。

设计要点：

- handoff 不是“写一段总结”，而是生成可恢复对象。
- compaction 之前必须做 blocker 抽取。
- `recommended_next` 要和 runtime 当前输出保持一致，不能各写各的。

### 10.6 技能入口路径：prompt 驱动的按需展开

目标：让 Skills 不再用大段上下文常驻，而是通过统一入口按需获取。

流程：

1. agent 通过 MCP `prompts/list` 看到可用技能入口。
2. 需要时调用 `prompts/get`。
3. registry 根据 `workflow/skills.spec.yaml` 和 skill frontmatter 产出 prompt 内容。
4. 只加载当前技能，而不是把所有技能塞进路由文档。

设计要点：

- `AGENTS.md` 只保留技能目录与触发规则。
- prompt 是用户可控入口，不应隐式篡改运行时状态。
- 技能文档应该服务于交互，而不是吞掉交互层。

---

## 11. Hooks 重构建议（交互导向）

### 11.1 Hook 必须变薄

Hook 脚本只保留四类职责：

1. 接收原生事件。
2. 做最小输入规范化。
3. 调用 Hook Client / Kernel。
4. 把 verdict 翻译回引擎原生格式。

Hook 不应该再承担：

- runtime 状态推理
- 策略合并
- 复杂批准逻辑
- 多文件语义分析
- 长文本生成

### 11.2 引入 `failure_mode`

建议在 `workflow/hooks.spec.yaml` 中显式增加每个 hook 的 `failure_mode`：

- `hard_gate`：失败默认阻塞
- `advisory`：失败允许降级，但必须记录
- `telemetry`：失败只影响证据，不影响主流程

建议映射：

- `PreToolUse:Write|Edit` → `hard_gate`
- `PreToolUse:Bash` → `hard_gate`
- `Stop` → `hard_gate`
- `PostToolUse:*` → `telemetry`
- `SessionStart` / `BeforeToolSelection` / `AfterModel` → `advisory`

### 11.3 允许窄输入变换，禁止语义重定向

可以允许的变换：

- 路径标准化
- 明确的字段补全
- approval token 注入
- engine-native 字段映射

禁止的变换：

- 擅自把任务改成另一个任务
- 把用户请求重定向成不同语义的操作
- 在 hook 层重新发明 planner

### 11.4 Hook 错误处理原则

- stdout 必须保持 JSON-only。
- 任何日志必须写 stderr 或 journal。
- Hook Client 必须能区分“策略拒绝”和“执行故障”。
- `hard_gate` 绝不能悄悄 fail-open。

---

## 12. MCP 重构建议（交互导向）

### 12.1 从工具服务器升级为交互总线

MCP 在本项目中不应只是“多暴露几个 tools”，而应承担四类交互职责：

1. **状态聚合面**：resources
2. **技能入口面**：prompts
3. **动作执行面**：tools
4. **用户输入回流面**：elicitation / subscriptions

### 12.2 资源层建议

在现有 `workflow://session`、`workflow://task-graph`、`workflow://journal`、`workflow://dashboard` 基础上，建议逐步增强：

- `workflow://approvals`
- `workflow://checkpoints`
- `workflow://blockers`
- `workflow://capability-gap`
- `workflow://trust`
- `workflow://handoff/latest`

同时建议支持：

- `resources/templates/list`
- `resources/subscribe`
- `resources/listChanged`

这样 agent 可以：

- 先读聚合状态；
- 再只订阅自己关心的交互对象；
- 在审批、问题、恢复状态变化时收到更新，而不是不断轮询。

### 12.3 Prompt 层建议

Prompt 层应成为轻量技能桥接层：

- `prompts/list` 提供技能入口目录；
- `prompts/get` 负责按需展开技能内容；
- prompt 参数必须显式；
- prompt 只组织输入，不直接篡改运行时。

### 12.4 Tool 层建议

Tool 层建议逐步标准化输出为：

- `content`：给模型看的简短文本
- `structuredContent`：给 runtime / adapter / other tools 消费的结构化对象

重点不是“返回更多文本”，而是“返回更稳定的结构”。

建议标准字段：

- `verdict`
- `reason_code`
- `blocking`
- `interaction_ref`
- `recommended_next`
- `artifacts`

### 12.5 Elicitation 与 fallback

- 支持的客户端：优先 MCP `elicitation`
- 不支持的客户端：退化为 JSONL request + resource 暴露
- 敏感输入必须使用安全输入模式
- 所有 elicitation 必须绑定 originating client request

### 12.6 Trust 模型建议

建议把 trust 做成可见对象，而不是散落在各配置里的隐式状态：

- hook trust
- MCP server trust
- tool-level trust
- folder/workspace trust

建议新增：`.ai/workflow/hook-trust.json`

最少要记录：

- trusted subject
- granted_by
- granted_at
- scope
- revoke_supported
- fingerprint / version

---

## 13. 跨引擎交互映射

| 交互议题 | Claude Code | Codex | Gemini CLI | 本项目建议 |
|---|---|---|---|---|
| 工具前拦截 | `PreToolUse` | 无等价 hooks，依赖 sandbox/approval | `BeforeToolSelection` / policy | 以 canonical preflight 抽象，adapter 各自映射 |
| 高风险审批 | permission + hooks | `approval_policy` | policy `ask_user` | 统一 approval_request 对象 |
| Stop/继续工作 | `Stop` hook | 无直接等价，靠 runtime + approval 约束 | `AfterAgent`/checkpoint 组合 | 统一为 runtime stop gate |
| 上下文压缩前收口 | `PreCompact` | 无原生等价 | checkpointing / compaction 语义 | 统一产出 handoff_capsule |
| 技能入口 | skills / local docs | `AGENTS.md` + skills | `GEMINI.md` + prompts | 都由 skills registry 编译 |
| 状态刷新 | hooks + local files | 重新读取 docs/config/MCP | resource + trust + checkpoint | 统一走 MCP resources + runtime store |
| 访问控制 | managed permission rules | sandbox + config | policy engine + trust | 原生能力优先，禁止伪统一 |

核心结论：

- **Claude** 适合做 managed hooks + managed permission rules。
- **Codex** 应坚持 config-first，不要硬做 hook bridge 幻觉。
- **Gemini** 必须尽量吃满原生 `BeforeToolSelection`、policy、checkpoint、trust 能力。

---

## 14. Integration Points

### 14.1 Runtime ↔ Hooks

- Runtime 提供统一 decision envelope。
- Hook Client 只传输标准化请求/响应。
- `workflow/hooks.spec.yaml` 负责事件矩阵与 failure_mode。

### 14.2 Runtime ↔ MCP

- Runtime 负责状态真相。
- MCP 负责暴露资源、prompts、tools、subscriptions、elicitation。
- MCP 不直接取代 runtime 决策。

### 14.3 Skills ↔ MCP

- `workflow/skills.spec.yaml` 提供技能注册信息。
- `seeyue-mcp/src/prompts/registry.rs` 负责把技能元数据映射为 prompts。
- Skills 只在需要时展开，不进入常驻路由上下文。

### 14.4 Router ↔ Interaction Layer

- router 产出 `recommended_next`。
- interaction layer 负责把它包装成用户可见动作。
- 任何 `recommended_next` 的展示都不得脱离 runtime 当前快照。

---

## 15. 分阶段实施建议

### P0：先收口交互契约与 Hook 边界

目标：先把最核心的交互语义稳定下来。

建议项：

1. 新增 `workflow/interaction.schema.yaml`
2. 给 `workflow/hooks.spec.yaml` 增加 `failure_mode`
3. 将 `scripts/runtime/hook-client.cjs` 明确为薄桥接层
4. 设计 `.ai/workflow/hook-trust.json`
5. 统一 approval / restore / question / input request 的字段命名

验收条件：

- runtime、hooks、MCP 三者能共享同一套交互对象定义
- `hard_gate` 不再 silently fail-open
- blocker-first 输出稳定可预测

### P1：把 MCP 升级为交互总线

目标：让 MCP 不只会“执行工具”，还能稳定传输交互状态。

建议项：

1. 扩展 dashboard 与交互型 resources
2. 增加 `listChanged` / `subscribe`
3. 标准化 tool `structuredContent`
4. 增加 elicitation 能力与 fallback 设计
5. 将 handoff/checkpoint 以资源对象暴露

验收条件：

- agent 不需要通过散读文件来判断 blocker
- approval / restore / question 状态可被订阅与结构化读取
- prompts/get 与 skills registry 行为一致

### P2：按引擎吃满原生交互能力

目标：把“交互一致”落在 adapter 层，而不是落在用户忍耐上。

建议项：

1. Claude：收敛到 managed hooks + managed permission rules
2. Codex：收敛到 `AGENTS.md` + config-first + MCP
3. Gemini：收敛到 `BeforeToolSelection` + policy + checkpoint + trust
4. 输出 capability-gap 报告，明确哪些语义是降级支持

验收条件：

- 三个引擎都能稳定表达 approval / restore / handoff / blocker
- 引擎差异只体现在 adapter，不体现在真相源定义

### P3：补齐 Skills 的交互集成面

目标：让 Skills 成为真正的交互编排层，而不是大段文档集合。

建议项：

1. 继续把 skills registry 作为唯一技能入口
2. 调整 skill 触发说明，让其面向交互契约
3. 把关键技能输出与 interaction schema 对齐
4. 让 skill compiler 输出更稳定的 prompt / artifact metadata

验收条件：

- Skills 不再依赖大篇幅常驻上下文
- 技能触发与 prompts/list/get 的行为一致
- 交互对象与技能编排对齐

---

## 16. Top Risks

1. **高风险：新增 interaction schema 后，现有 JSONL/状态文件短期会并存**  
   缓解：先把 schema 作为契约层引入，不立刻大规模迁移存储实现。

2. **高风险：Codex 与 Claude/Gemini 的交互边界差异较大，容易再次回到伪统一**  
   缓解：坚持 adapter absorb differences，禁止把 Codex 强行改造成 hook-first 模型。

3. **中风险：Hooks 薄化后，Kernel 会短期承受更多职责，测试压力上升**  
   缓解：把 decision envelope 与 interaction schema 一起固化，先补 contract tests，再做大迁移。

4. **中风险：MCP 订阅、elicitation、trust 增强后，服务端复杂度会上升**  
   缓解：先做最小闭环，只覆盖 blocker-first 交互，再逐步扩展到更丰富场景。

---

## 17. 验收标准

满足以下条件，才算本轮重构真正成功：

- 用户能在任意引擎中快速看懂当前 blocker、原因、影响范围、下一步。
- 所有审批、恢复、问题请求都具备统一结构化对象。
- `recommended_next` 与 `restore_reason` 只来自 runtime，不来自自由文本推断。
- Hooks 不再承载主策略，MCP 不再只是工具壳。
- 上下文压缩与 handoff 后，agent 能依靠 capsule 恢复，而不是依赖聊天历史猜测。
- 引擎差异主要体现在 adapter，而不是体现在状态语义分叉。

---

## 18. 一句话结论

本项目下一阶段的关键，不是“把 Hooks 做得更重”或“把 MCP tools 做得更多”，而是把 **interaction 设计成独立的一等公民**：

- Runtime 决策
- Hooks 守边界
- MCP 做总线
- Skills 做编排

只有先把交互契约统一，后续的多引擎 workflow 体系才会真正稳定、可解释、可恢复、可扩展。
