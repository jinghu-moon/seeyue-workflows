# V4 格式分工说明

状态：说明文档  
定位：解释 `seeyue-workflows` V4 为什么采用 `Markdown + YAML + JSONL + Code` 的格式分工  
读者：维护者、规范编写者、adapter / validator / hook 开发者

## 1. 一句话结论

V4 不追求“全仓库只用一种格式”，而是按资产类型选择最合适的格式：

- `Markdown`：给人讨论、审核、解释
- `YAML`：给人编写、给机器读取的声明式规格
- `JSONL`：给运行时追加写事件日志
- `Code`：给真正的执行逻辑、校验器、hook、adapter

这不是“技术偏好”，而是职责分离。

## 2. 为什么不是一种格式通吃

不同文件承担的职责完全不同：

- 有些文件是“让人读懂和拍板”的
- 有些文件是“让 runtime 稳定解析”的
- 有些文件是“让事件不断追加记录”的
- 有些文件本身就是程序逻辑

如果强行统一成一种格式，通常会出现两个问题：

- 要么人类维护体验很差
- 要么机器消费不稳定

因此 V4 采用分层格式策略，而不是统一格式策略。

## 3. 为什么 `docs/*.md` 用 Markdown

`Markdown` 适合：

- 架构讨论
- 审核稿
- 解释性文档
- 迁移说明
- 设计取舍记录

例如：

- `docs/architecture-v4.md`
- `docs/archive/outdated/router-spec-draft.md`
- `docs/archive/outdated/test-gate-spec-v2.md`

原因：

- 人类阅读成本低
- 结构清晰
- 适合写背景、例子、边界、取舍
- 适合进行逐段审核和批注

不适合直接作为机器真源的原因：

- 结构过于宽松
- 解析稳定性差
- 难以做严格 schema 校验
- 很容易把“解释文字”与“规则本体”混在一起

结论：

- `Markdown` 是人类审阅层
- 不是机器真源层

## 4. 为什么 `workflow/*.yaml` 用 YAML

`YAML` 适合：

- 有层级的规则文件
- 人机共同维护的配置 / 规格
- 需要后续进入 validator / adapter / generator 的声明式真源

例如：

- `workflow/runtime.schema.yaml`
- `workflow/router.spec.yaml`
- `workflow/policy.spec.yaml`

原因：

- 比 JSON 更适合人类阅读和手工修改
- 比 Markdown 更适合机器稳定解析
- 比 TOML 更适合深层嵌套结构
- 比直接写代码 DSL 更容易保持“声明式”边界

对于 V4 这种规格，典型内容包括：

- phase / node 结构
- route rules
- policy gates
- approval matrix
- capability / persona bindings

这些内容都天然适合层级结构表达，因此 YAML 是合理选择。

## 5. 为什么不是 JSON

JSON 的优点：

- 机器解析稳定
- 生态广
- 工具链成熟

但对 V4 规格来说，它有几个明显缺点：

- 手工维护体验差
- 可读性弱于 YAML
- 无注释能力
- 对长规则文件不友好

所以 JSON 适合：

- 机器内部对象
- API 交换
- 中间转换格式

不适合做“长期由人维护的主规格文件”。

## 6. 为什么不是 TOML

TOML 很适合：

- 扁平配置
- 小型项目设置文件
- 简单键值结构

但不太适合 V4 这种：

- 深层嵌套
- 数组对象较多
- 规则结构复杂
- phase / node / rule 模板并存

因此 TOML 不是当前最优解。

## 7. 为什么 `journal` 用 JSONL

运行时日志和规格文件不是一类资产。

`journal.jsonl` 适合：

- 逐行追加
- 单事件写入
- 审计与回放
- 崩溃恢复
- 事件流分析

原因：

- 每行一个 JSON object，易于 append-only 写入
- 易于流式处理
- 易于 grep / parser / replay 工具消费
- 比 YAML 更适合事件日志

因此：

- 规格用 YAML
- 事件日志用 JSONL

## 8. 为什么执行逻辑必须是代码

有些东西不能长期停留在 YAML 或 Markdown 中，例如：

- hook 拦截逻辑
- validator 计算逻辑
- route 规则解释器
- adapter 生成逻辑
- TDD / approval / recovery 的真实执行器

这些能力必须用代码实现，例如：

- Python
- Node.js
- PowerShell（必要时）

原因：

- 它们不是“描述”，而是“执行”
- 需要异常处理、I/O、错误分类、兼容性处理
- 需要测试、调试和回归验证

结论：

- YAML 描述规则
- Code 执行规则

## 9. 推荐的格式职责分工

### 9.1 人类审核层

使用：`Markdown`

作用：

- 架构说明
- 评审草案
- 示例
- 背景解释
- 取舍记录

### 9.2 机器真源层

使用：`YAML`

作用：

- runtime schema
- router spec
- policy spec
- capability / persona binding

### 9.3 运行时事件层

使用：`JSONL`

作用：

- append-only journal
- replay
- audit trail
- resume evidence

### 9.4 执行逻辑层

使用：`Code`

作用：

- hooks
- validators
- adapters
- policy enforcement
- route evaluation

## 10. 使用 YAML 时的约束

YAML 虽然适合规格文件，但也有典型风险：

- 缩进敏感
- 隐式类型容易出坑
- anchors / aliases / merge key 过于“魔法”
- 不同解析器可能有细节差异

因此 V4 建议采用 YAML 安全子集：

- 禁止 anchors / aliases / merge keys
- 不依赖隐式布尔值和隐式日期
- 所有关键字段做显式 schema 校验
- 运行时内部可转换为标准对象结构

## 11. 这套分工对 V4 的意义

采用 `Markdown + YAML + JSONL + Code` 的组合后，V4 可以同时满足：

- 人能审
- 机器能读
- 状态能恢复
- 日志能回放
- 规则能执行

这也是为什么 V4 更接近一个：

- Agent workflow control plane
- 而不是单纯 prompt collection

## 12. 当前建议

对 `seeyue-workflows`，建议继续坚持：

- `docs/*.md`：人类审阅层
- `workflow/*.yaml`：机器真源层
- `.ai/workflow/journal.jsonl`：运行时事件层
- `scripts/`、`hooks/`、validator / adapter：执行逻辑层

不要把这几层混在一起。

## 13. 后续可扩展建议

后续如果 V4 继续演进，可以进一步形成：

- `docs/`：讨论和审阅
- `workflow/`：声明式规格
- `.ai/workflow/`：运行态状态与事件
- `scripts/` / `hooks/`：执行与 enforcement

这样可以长期保持边界清晰，避免“规则写在文档里、逻辑写在 prompt 里、状态丢在聊天里”的混乱情况。
