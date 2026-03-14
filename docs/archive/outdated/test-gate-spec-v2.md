# 测试门规范草案 v2

状态：人类审阅稿  
版本：v2  
定位：面向 `seeyue-workflows` V4 的测试门策略草案，用于后续收敛到 `workflow/policy.spec.yaml`

## 1. 文档目标

这份草案用于回答 4 个问题：

- 什么时候允许开始写实现代码
- 什么时候一个 node 可以算完成
- 什么时候一个 phase 可以放行
- 什么时候整个会话可以安全结束

本草案是给人审阅和拍板的，不是最终机器规格。最终机器可执行规则应下沉到 `workflow/policy.spec.yaml` 与相关 validator。

## 2. 核心设计原则

### 2.1 测试合同先于测试框架

不同项目可以使用不同语言与测试工具，但必须遵守统一的测试合同：

- RED：先写失败测试，并确认失败原因正确
- GREEN：再写最小实现，使测试通过
- VERIFY：最后做节点级、阶段级和会话级验证

统一的是测试语义，不是单一工具链。

### 2.2 行为优先于实现

测试应证明系统行为和契约，而不是证明某个内部函数或 mock 被调用。

禁止把以下情况作为主要完成依据：

- 只验证 mock 调用次数
- 只验证内部私有实现细节
- 通过过度简化的 mock 制造“假绿”

### 2.3 覆盖率是门槛，不是结论

覆盖率只代表代码被执行到，不代表行为已经被正确验证。

因此：

- 覆盖率通过，但行为门失败，不能放行
- 覆盖率不足，即使测试通过，也不能按要求完成 node

### 2.4 测试门必须可审计

所有 RED / GREEN / VERIFY 结论都必须有证据支撑，并能写入 runtime 事件与状态中。

## 3. 测试门的三层结构

### 3.1 Node Gate

Node Gate 负责判断：

- 当前 node 是否允许开始实现
- 当前 node 是否允许标记为完成

### 3.2 Phase Gate

Phase Gate 负责判断：

- 当前 phase 是否允许结束
- 是否允许推进到下一 phase

### 3.3 Stop Gate

Stop Gate 负责判断：

- 当前会话是否允许宣布完成
- 是否允许在没有遗留风险的情况下停止

## 4. Node Gate 规范

### 4.1 适用范围

以下类型默认必须进入 Node Gate：

- 新功能
- Bug 修复
- 行为变更
- 有行为风险的重构
- API / hook / policy / router / adapter 变更

以下情况默认不强制 TDD，但仍需最小验证：

- 纯文档变更
- 纯注释变更
- 纯格式化变更
- 纯脚手架占位资源

### 4.2 Node 需要声明的最小测试信息

每个行为变更 node 建议至少声明：

- `layer`
- `coverage_mode`
- `coverage_profile`
- `mock_policy`
- `acceptance_criteria_refs`
- `red_cmd`
- `green_cmd`
- `red_expectation`
- `behavior_gate`

并在 `node.verify` 中声明：

- `cmd`
- `pass_signal`

推荐语义：

- `layer`：`unit | integration | contract | e2e | smoke`
- `coverage_mode`：`full | patch`
- `coverage_profile`：`critical | core | standard | utility | scaffold`
- `mock_policy`：`none | boundary_only | allowed_with_justification`
- `red_expectation`：`allowed_failure_kinds / rejected_failure_kinds / allowed_exit_codes(可选) / stderr_pattern(可选) / error_type(可选)`
- `behavior_gate`：`ac_traceability_required`（可选 `boundary_conditions_required`）

### 4.3 RED 门

如果 `tdd_required=true`，则必须先经过 RED 门。

RED 门的合法条件：

- 执行了 `red_cmd`
- 测试确实失败
- 失败原因对应缺失行为，而不是测试本身损坏
- RED 证据被记录

未满足以上条件时：

- 不允许写生产代码
- `pre-write` 应阻断继续写入

### 4.4 防“假失败”规则

“测试失败”不等于“合法 RED”。

合法 RED 应属于以下类别之一：

- 断言失败
- 契约不匹配
- 预期校验失败
- 与缺失功能直接相关的结果不匹配

默认不接受以下类别作为 RED：

- 语法错误
- 导入错误
- 环境启动失败
- 数据库/网络不可达
- 权限错误
- 与目标行为无关的 fixture 初始化错误

建议为 RED 证据增加以下约束：

- `exit_code`
- `failure_kind`
- `allowed_failure_kinds`
- `rejected_failure_kinds`
- `allowed_exit_codes`（可选）
- 必要时增加 `stderr_pattern` 或 `error_type`

原则：

- 只有命中允许的失败类型，RED 才成立
- 默认不允许把环境故障伪装成 RED

### 4.5 GREEN 门

GREEN 的目标不是“写完代码”，而是“写出最小通过实现”。

GREEN 成立的条件：

- `green_cmd` 通过
- 与当前 node 直接相关的目标测试通过
- 没有引入新的阻断错误

### 4.6 Behavior Gate

`behavior_gate=pass` 必须建立在可追溯的验收标准上，而不是主观判断。

规则建议：

- 每个行为变更 node 必须绑定明确的 `Acceptance Criteria`（AC）
- 每条 AC 必须至少对应一个测试
- 每个通过的测试必须能追溯到对应 AC
- 只有所有必需 AC 都被测试覆盖并通过时，`behavior_gate` 才能为 `pass`

推荐做法：

- node 声明 `acceptance_criteria_refs`
- 测试命名或元数据中显式标记 `AC-1`、`AC-2` 等引用

不建议只依赖模糊的测试名自然语言匹配。

### 4.7 Node 完成条件

一个 node 只有在以下条件全部满足时，才能标记为 `completed`：

- RED 已合法观察
- GREEN 已通过
- `node.verify.cmd` 已通过且命中 `pass_signal`
- `behavior_gate=pass`
- 达到覆盖率要求
- 所需 review 已通过
- 没有待处理审批

## 5. Phase Gate 规范

### 5.1 Phase 的职责

Phase 不负责替代 node 的细粒度测试。

Phase 的职责是：

- 汇总阶段内 node 的完成情况
- 执行阶段级退出验证
- 决定能否进入下一个 phase

### 5.2 Phase 完成条件

一个 phase 只有在以下条件全部满足时，才能标记为 `completed`：

- 该 phase 下所有必需 node 已完成
- 没有 `failed` / `rework` 未处理 node
- `exit_gate` 对应命令通过
- 不存在待处理审批
- 不存在未恢复的中断状态

### 5.3 Phase 并行策略

V4 第一版建议：

- Phase 默认串行
- 不启用并行 phase
- 若未来引入并行 phase，必须额外定义审批、预算、恢复和冲突规则

## 6. Stop Gate 规范

Stop Gate 用于防止“差不多了就结束”。

以下任一情况存在时，不允许宣布完成：

- 仍有 `pending approval`
- 审批队列超过 `max_pending_approvals`
- 当前 node 未完成验证
- 当前 phase 未通过 `exit_gate`
- review 证据缺失或过期
- runtime 状态字段不完整
- 预算已耗尽但未进入人工接管

## 7. 测试层级规范

### 7.1 Unit

适用对象：

- 纯函数
- 规则判断
- 小型状态转换
- 工具函数

要求：

- 快
- 稳定
- 独立
- 适合作为 RED 的第一步

### 7.2 Integration

适用对象：

- API
- 数据库交互
- 服务协作
- hook 与 runtime 联动
- adapter 行为

要求：

- 验证真实接口或接近真实接口
- 不只看状态码，还要看结构、错误语义、边界行为

### 7.3 Contract

适用对象：

- schema
- validator
- router 输入/输出
- policy 判定
- adapter 生成物

要求：

- 给定输入状态，断言输出决定或结构合法性
- 特别适合 `recommended_next`、审批对象、事件格式验证

### 7.4 E2E

适用对象：

- 关键用户或维护者流程
- 完整 workflow 主链

要求：

- 只覆盖关键路径
- 不重复低层测试已充分验证的内容

### 7.5 Smoke

适用对象：

- hook 基础可运行性
- adapter 基础可用性
- 核心命令链路未崩

要求：

- 成本低
- 执行快
- 适合阶段出口与发布前检查

## 8. 覆盖率策略

### 8.1 风险驱动覆盖率基线

建议按风险等级设置最低覆盖率：

- `critical`: `100%`
- `core`: `90%`
- `standard`: `80%`
- `utility`: `60%`
- `scaffold`: `not enforced`

### 8.2 Legacy Code 的 Patch Coverage

遗留系统不应被“一次性补齐全文件覆盖率”绑架。

对于历史覆盖率很低的模块，默认采用 `patch_coverage` 策略：

- 本次新增或修改的代码必须达到目标覆盖率
- 不得降低全局覆盖率
- 不得降低被触达区域的已有覆盖率
- Bug 修复必须补 characterization test 或 reproducer test

这条规则的目标是：

- 控制 node 范围
- 防止为了补历史债务导致本次变更无限膨胀
- 让遗留系统也能渐进式进入 V4

### 8.3 覆盖率不是行为门替代品

即使覆盖率达到要求，以下情况仍不能放行：

- AC 未全部映射到测试
- 关键边界行为未验证
- 只通过 mock-only 测试获得高覆盖率

## 9. Mock 使用策略

### 9.1 允许 mock 的场景

- 第三方 API
- 时间/随机数
- 难稳定控制的网络失败
- 高成本外部资源
- 明确的边界依赖

### 9.2 不建议 mock 的场景

- 自身核心业务逻辑
- 状态机核心路径
- router 判定核心路径
- policy 阻断核心路径

### 9.3 禁止的完成依据

以下情况不得作为 node 完成依据：

- 只断言 mock 被调用几次
- 只断言内部实现细节
- 用过度简化的 mock 掩盖真实契约问题

## 10. 审批疲劳与低风险放行

### 10.1 目标

V4 需要降低审批疲劳，但不能回退到“凭感觉自动放行”。

### 10.2 不建议采用主观 Confidence Score

不建议把 agent 的主观 `confidence score` 作为主要放行依据。

原因：

- 不可审计
- 不可复算
- 容易漂移
- 与“evidence over claims”原则冲突

### 10.3 建议采用 Deterministic Notify-Only Eligibility

建议引入一种可规则化的低风险放行模式：`notify_only`。

只有同时满足以下条件时，才允许进入 `notify_only`：

- 风险等级为 `low`
- 变更类型为 `docs` / `scaffold` / `utility`
- 不涉及 destructive command
- 不涉及 git 变异操作
- 不涉及 auth / security / secrets
- 不涉及 schema / public API
- 不涉及数据迁移
- 不存在 `tdd_exception`
- 所需验证已通过
- 变更可完整写入 `journal` 与 `ledger`

原则：

- `notify_only` 是规则放行，不是主观放行
- 它只适用于低风险、可审计、可回溯的小变更
- 绝不能扩展到危险命令、安全相关或数据相关操作
- 它不能重定义 `grant_scope`，也不能替代 `approval_mode`

## 11. Workflow 专用测试类型

除通用单元/集成/E2E 外，`seeyue-workflows` 建议增加 4 类专用测试：

- Hook Policy Tests
  - 验证 `pre-write` / `pre-bash` / `stop` 是否正确阻断
- Router Decision Tests
  - 给定 runtime 状态，断言 `recommended_next`
- Schema / Validator Tests
  - 非法状态、非法审批对象、非法 TDD 转移必须被拒绝
- Human Output Golden Tests
  - 审批提示、阻塞提示、澄清提示必须符合固定格式

若进入 V4 的最小执行韧性能力，建议追加：

- Execution Resilience Tests
  - 验证 `retry_policy` / `timeout_policy` 的最小字段约束
  - 验证超时后必须先记录 `node_timed_out`，再进入后续路由

## 12. 证据要求

每个受测试门约束的 node，至少应能产出：

- `RED`
  - 执行命令
  - 失败时间
  - 失败类型
  - 失败原因
- `GREEN`
  - 执行命令
  - 通过范围
- `VERIFY`
  - 验证命令
  - 通过信号
- `COVERAGE`
  - 实际覆盖率
  - 目标覆盖率
  - 覆盖率模式（`full` 或 `patch`）
- `BEHAVIOR`
  - AC 覆盖情况
  - 边界场景是否通过

当 node 声明 `timeout_policy` 时，还应补充：

- `TIMEOUT`
  - 超时阈值
  - `node_timed_out` 事件记录
  - 超时后的路由结果（`fail_node` / `block_node` / `require_human`）

证据缺失时：

- node 不得完成
- phase 不得放行
- stop 不得结束会话

## 13. 例外机制

以下情况可以申请测试例外，但必须显式记录并获得人工批准：

- 纯文档改动
- 纯格式化改动
- 纯注释改动
- 脚手架资源
- 当前阶段确实无法建立可执行测试，但存在可接受的替代验证方式

例外记录至少包含：

- `reason`
- `alternative_verification`
- `user_approved=true`

没有明确批准，不得绕过测试门。

## 14. 建议固化为机器规格的重点

后续进入 `workflow/policy.spec.yaml` 时，建议优先固化以下内容：

- Node Gate / Phase Gate / Stop Gate 的判定条件
- `patch_coverage` 与覆盖率非回退规则
- RED failure classification 规则
- AC 到测试的 traceability 规则
- `notify_only` 的确定性放行条件
- mock-only assertion 禁止作为完成依据
- `retry_policy` / `timeout_policy` 的最小字段契约
- `node_timed_out` 事件与后续路由语义

## 15. 审核建议

建议你重点看以下 5 个是否符合你的预期：

- 是否接受 `patch_coverage` 作为遗留系统默认策略
- 是否接受 RED failure classification 作为硬门
- 是否接受 AC traceability 作为 `behavior_gate` 的客观依据
- 是否接受 Phase 第一版串行
- 是否接受 `notify_only` 替代主观 confidence score
