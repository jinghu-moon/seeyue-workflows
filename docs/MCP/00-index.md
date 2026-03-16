# seeyue-workflows MCP 集成方案文档索引

> 本目录是 seeyue-workflows MCP 集成的完整方案文档集。
> 所有内容基于项目现有源码、官方 MCP 规范源码（`refer/mcp-source/modelcontextprotocol-main/`）和 MCP-DEMO 参考实现，禁止无来源推断。

---

## 文档结构

| 文件 | 内容 | 目标读者 |
|------|------|----------|
| [01-protocol.md](./01-protocol.md) | MCP 协议基础：角色、生命周期、原语、传输层、2025-11-25 新增内容 | 所有开发者 |
| [02-architecture.md](./02-architecture.md) | seeyue-mcp 整体架构：模块划分、AppState、IPC 桥接、多引擎互操作 | 架构师 |
| [03-file-editing-engine.md](./03-file-editing-engine.md) | 文件编辑引擎：MCP-DEMO 深度分析、V5 五工具、V8 扩展工具 | 后端开发者 |
| [04-hooks-integration.md](./04-hooks-integration.md) | Hooks 系统 MCP 化：verdict 枚举、工具规格、四层分离架构 | 后端开发者 |
| [05-workflow-resources.md](./05-workflow-resources.md) | Workflow 状态作为 MCP Resources：资源清单、订阅通知、并发控制 | 后端开发者 |
| [06-skills-as-prompts.md](./06-skills-as-prompts.md) | Skills 系统作为 MCP Prompts：映射原理、prompts/get 实现 | 技能开发者 |
| [07-windows-native.md](./07-windows-native.md) | Windows 原生优化层：路径规范化、编码检测、IOCP | Windows 平台开发者 |
| [08-implementation-plan.md](./08-implementation-plan.md) | 实施路线图与优先级：P0/P1/P2 交付计划 | 项目管理 |

---

## 核心参考资料

| 来源 | 路径 | 用途 |
|------|------|------|
| MCP 规范 2025-11-25 | `refer/mcp-source/modelcontextprotocol-main/docs/specification/2025-11-25/` | 协议标准（当前最新稳定版）|
| MCP Schema（TypeScript 真实来源）| `refer/mcp-source/modelcontextprotocol-main/schema/2025-11-25/schema.ts` | 协议结构定义 |
| MCP 架构概述 | `refer/mcp-source/modelcontextprotocol-main/docs/docs/learn/architecture.mdx` | Host/Client/Server 角色 |
| MCP-DEMO Rust 实现 | `refer/MCP-DEMO/` | 文件编辑引擎参考实现 |
| V5 设计文档 | `refer/MCP-DEMO/V5-DESIGN.md` | Rust MCP Server 设计原则 |
| V8 设计文档 | `refer/MCP-DEMO/Agent-File-Editing-Engine-v8.md` | 17 工具完整规格 |
| Claude Code 源码 | `refer/agent-source-code/claude-code-main/` | hooks 事件参考、PreToolUse input-rewriting 示例 |
| Gemini CLI 源码 | `refer/agent-source-code/gemini-cli-main/` | 四层 hooks 架构、Policy Engine、Scheduler、SkillManager 参考 |
| Codex 源码 | `refer/agent-source-code/codex-main/` | Rust app-server 传输层、AGENTS.md 层级加载、skills schema 参考 |
| Claude Code Security Review | `refer/agent-source-code/claude-code-security-review-main/` | 两阶段过滤架构、>80% 置信度阈值、大 diff 降级策略参考 |
| rmcp SDK | `https://github.com/modelcontextprotocol/rust-sdk` | 官方 Rust MCP SDK |

---

## 规范版本说明

| 版本 | 状态 | 关键变化 |
|------|------|----------|
| 2024-11-05 | 历史版本 | 初始版本，HTTP+SSE 传输 |
| 2025-03-26 | 历史版本 | 引入 Streamable HTTP（取代 HTTP+SSE）|
| 2025-06-18 | 历史版本 | rmcp SDK 当前协商版本（`ProtocolVersion::V_2025_06_18`）|
| **2025-11-25** | **当前最新** | 新增 icons/title/outputSchema/Tasks(实验)/Elicitation URL mode/JSON Schema 2020-12 默认方言 |
| draft | 开发中 | 未发布 |

---

## 设计约束

1. **Windows 专项**：100% 专注 Windows 平台，不追求跨平台兼容
2. **来源可追溯**：所有设计决策必须有源码或官方文档依据
3. **渐进实施**：P0 → P1 → P2 分优先级交付，不破坏现有 hooks 行为
4. **向后兼容**：MCP Server 作为新增层，不替换现有 `hook-client.cjs` 逻辑
5. **SDK 限制知悉**：rmcp 当前协商版本为 `2025-06-18`，规范最新为 `2025-11-25`
6. **三层协作架构**：MCP / Skills / Hooks 各司其职，互相配合而非单一替代——这是项目后续开发的核心方向，详见 `08-implementation-plan.md` §6
