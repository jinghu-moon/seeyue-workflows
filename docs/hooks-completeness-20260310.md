# Hooks 完备性结论与 v3/v4 对比（2026-03-10）

## 结论

- V4 已建立 hooks 的机器源规范（`workflow/hooks.spec.yaml`），并将命令分类纳入同一规范，形成“规范 → 适配器 → 引擎配置”的闭环。
- Claude Code 与 Gemini CLI 的 hooks 已按 V4 hook matrix 全量落地（见 `.claude/settings.json` 与 `.gemini/settings.json`）。
- Codex 端没有原生 hooks，当前以 instruction/sandbox bridge 方式承接（见 `scripts/adapters/compile-adapter.cjs`），因此硬拦截能力不及 Claude/Gemini。

## v3 vs v4 Hooks 对比表

| 维度 | v3 设计 | v4 当前实现 | 结论 |
| --- | --- | --- | --- |
| 规范载体 | v3 文档内给出 `.claude/settings.json` 示例（`refer/workflow-skills-system-design-v3.md`） | 独立 machine source `workflow/hooks.spec.yaml` | V4 规范化更强 |
| Hook 事件集 | SessionStart、PreToolUse(Bash/Write|Edit)、PostToolUse(Write|Edit)、Stop | 在 v3 基础上新增 UserPromptSubmit、PostToolUse(Bash)、PreToolUse:Write 会话守卫、Bash 预算守卫 | V4 为 v3 超集 |
| 命令分类 | 未定义统一分类 | 在 hooks spec 内提供 `command_classification` | V4 完备 |
| Claude 接入 | 仅文档示例 | settings 已落地（`.claude/settings.json`） | 完整 |
| Gemini 接入 | 未覆盖 | settings 已落地（`.gemini/settings.json`，通过 bridge） | 完整 |
| Codex 接入 | 未覆盖 | 无 native hooks，桥接策略 | 受限 |

## 备注

- 若需全引擎一致的硬拦截，需要 Codex 侧引入可执行 hooks 或等效的 runtime 代理层拦截。当前桥接方案依赖指令与沙箱策略，无法达到 Claude/Gemini 的同级别拦截强度。
