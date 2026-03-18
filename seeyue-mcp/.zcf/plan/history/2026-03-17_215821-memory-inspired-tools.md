# Plan: Memory-Inspired Tools

## Context
基于 Memory-Palace / nocturne_memory 分析，为 seeyue-mcp 新增能力。

## Tasks
- P0: session_context 字段分离（router.rs disclosure 原则）
- P1: compact_journal 工具
- P2: search_session 工具
- P3: SKILL.md 关键词触发路由表

## Files
- seeyue-mcp/src/hooks/router.rs
- seeyue-mcp/src/tools/compact_journal.rs (new)
- seeyue-mcp/src/tools/search_session.rs (new)
- seeyue-mcp/src/lib.rs
- seeyue-mcp/src/main.rs
- seeyue-mcp/tests/test_compact_journal.rs (new)
- seeyue-mcp/tests/test_search_session.rs (new)
- .agents/skills/sy-mcp/SKILL.md
