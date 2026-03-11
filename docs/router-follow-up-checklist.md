# Router 后续任务清单（参考主流 Agent / Workflow 项目）

状态：后续任务参考资料  
定位：用于指导 `router / runtime / policy / capability` 后续演进，不是最终机器规格

## 1. 使用方式

这份清单用于回答两个问题：

- 哪些能力应该立刻进入 V4 主线
- 哪些能力应该拆到后续规格，而不是硬塞进 `router.spec.yaml v1`

建议采用三档优先级：

- `P0`：立即纳入当前主线
- `P1`：下一阶段纳入
- `P2`：延后到 V2 或并行能力阶段

## 2. 总体策略

### 2.1 立即纳入的原则

仅把真正属于 Router 决策层的能力放入 `router.spec.yaml v1`，例如：

- formal route rules
- capability-aware routing
- node priority
- minimal conditional routing
- machine-readable `recommended_next`

### 2.2 改造后纳入的原则

以下能力值得做，但应拆到合适规格：

- node action schema
- retry / backoff
- timeout / deadline
- capability registry

### 2.3 延后的原则

以下能力不建议过早引入，以免削弱可审计性：

- numeric route score
- true multi-active-node scheduling
- parallel phase execution

## 3. 任务清单

| ID | 优先级 | 任务 | 目标文件 | 说明 |
|---|---|---|---|---|
| R1 | P0 | Formalize `PhaseReady / PhaseDone / NodeReady / NodeDone` | `workflow/router.spec.yaml` | 把中文逻辑收敛成机器规则对象 |
| R2 | P0 | Upgrade `recommended_next` to machine schema | `workflow/router.spec.yaml` | 使用 `type / target / params / reason / priority` |
| R3 | P0 | Add `next_capability` routing output | `workflow/router.spec.yaml` | 让 Router 输出 capability，而不是直接输出具体 tool |
| R4 | P0 | Add `priority`-based ready set ordering | `workflow/router.spec.yaml` + `workflow/runtime.schema.yaml` | 优先用显式 priority 和稳定 tie-breaker，不急于引入 score |
| R5 | P0 | Add minimal conditional node semantics | `workflow/router.spec.yaml` + `workflow/runtime.schema.yaml` | 最小版条件路由，仅允许引用 durable state / policy verdict |
| R6 | P0 | Add `phase_completed` and `node_bypassed` events | `workflow/runtime.schema.yaml` + router logic | 增强审计和 replay 能力 |
| C1 | P1 | Introduce capability registry | `workflow/persona-bindings.yaml` 或 `workflow/capabilities.yaml` | 建立 `node -> capability -> persona` 映射 |
| C2 | P1 | Separate capability from concrete tool schema | `workflow/capabilities.yaml` | 能力层只描述类别、约束、绑定，不直接写 shell/tool 细节 |
| E1 | P1 | Define node `inputs / actions / outputs` schema | `workflow/execution.spec.yaml` 或 `workflow/runtime.schema.yaml` | 让 node 从“黑盒任务”演进为“可审计 task” |
| P1 | P1 | Add retry / backoff policy | `workflow/policy.spec.yaml` + runtime sync | 建议包含 `max_attempts / backoff / retryable_failure_kinds` |
| P2 | P1 | Add timeout / deadline policy | `workflow/policy.spec.yaml` + runtime sync | 建议至少支持 node timeout 和 phase deadline |
| P3 | P1 | Add policy-aware retry gating | `workflow/policy.spec.yaml` | 区分可重试错误和不可重试错误 |
| O1 | P1 | Expand route basis for replay/debug | `workflow/router.spec.yaml` | 记录排序决策、被排除候选、policy 裁决来源 |
| F1 | P2 | True multi-active node scheduling | `workflow/router.spec.yaml` + runtime redesign | 当前 runtime 只有单 `active_id`，不适合直接并行 |
| F2 | P2 | Parallel phase execution | `workflow/router.spec.yaml` + policy redesign | 需额外定义审批、预算和冲突边界 |
| F3 | P2 | Numeric route score | `workflow/router.spec.yaml` | 等显式规则稳定后，再考虑作为二级排序机制 |

## 4. 建议的落地顺序

### 第一批（紧贴当前主线）

1. `router.spec.yaml`
   - formal route rules
   - machine `recommended_next`
   - capability-aware output
   - priority ordering
   - minimal conditional routing
2. `runtime.schema.yaml`
   - sync `capability`
   - sync `priority`
   - sync `condition`
   - add `phase_completed` / `node_bypassed`

### 第二批（拆分能力层与策略层）

3. `workflow/persona-bindings.yaml` 或 `workflow/capabilities.yaml`
4. `workflow/policy.spec.yaml`
   - retry
   - timeout
   - retry gate
   - notify-only / approval relief
5. 可选：`workflow/execution.spec.yaml`
   - actions / inputs / outputs

### 第三批（未来扩展）

6. true parallel node scheduling
7. phase parallelism
8. numeric route score

## 5. 采纳边界建议

### 5.1 应直接采纳到 Router V1 的

- capability-aware routing
- formal route rules
- machine `recommended_next`
- node priority
- minimal conditional node

### 5.2 应拆到其他规格的

- capability registry
- tool/action schema
- retry policy
- timeout / deadline

### 5.3 应延后的

- numeric route score
- true parallelism
- phase parallelism

## 6. 参考主流项目的映射

| 主题 | 建议能力 | 可借鉴项目 |
|---|---|---|
| capability / tool abstraction | `node -> capability -> persona/tool` | OpenAI Agents SDK, CrewAI |
| typed node / typed edge | machine-readable `recommended_next` / formal route rules | OpenAI Agent Builder, LangGraph |
| conditional routing | minimal `condition` semantics | LangGraph |
| retries / backoff | retry policy and retryable errors | Temporal, Airflow |
| timeout / deadlines | node timeout / phase deadline | Temporal, Airflow |
| stateful orchestration | state-over-chat, durable route basis | CrewAI Flows, Temporal |

## 7. 官方参考资料

以下链接适合作为后续任务的参考资料：

- OpenAI Agents SDK — Agents / Tools
  - https://platform.openai.com/docs/guides/agents-sdk
  - https://openai.github.io/openai-agents-js/guides/tools/
- OpenAI Agent Builder — typed workflow nodes
  - https://platform.openai.com/docs/guides/agent-builder
- LangGraph — graph API / conditional edges
  - https://docs.langchain.com/oss/python/langgraph/graph-api
  - https://docs.langchain.com/oss/python/langgraph/use-graph-api
- CrewAI — flows / tasks / tools / persistence
  - https://docs.crewai.com/
  - https://docs.crewai.com/core-concepts/Tools/
- Temporal — durable execution / retry / timeout
  - https://docs.temporal.io/
  - https://api-docs.temporal.io/
- Airflow — task dependencies / retries / execution timeout
  - https://airflow.apache.org/docs/apache-airflow/stable/core-concepts/tasks.html
  - https://airflow.apache.org/docs/apache-airflow/stable/tutorial/taskflow.html

## 8. 建议你下一步拍板的 6 项

建议你优先确认：

- 是否新增 `workflow/capabilities.yaml`
- `capability` 是独立文件，还是并入 `workflow/persona-bindings.yaml`
- `condition` 是否先做最小版，只允许 state-based expression
- `retry / timeout` 是直接并入 `policy.spec.yaml`，还是先写成附录草案
- 是否新增 `workflow/execution.spec.yaml`
- 是否明确将 numeric route score 延后到 V2
