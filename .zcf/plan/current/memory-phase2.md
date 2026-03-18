# Plan: Memory-Inspired Tools Phase 2

## Tasks
- P0-1: session_summary 附加 recent_events
- P0-2: auto-flush (posttool_write 触发 compact)
- P2-2: posttool_write 记录 before/after hash
- P2-1: search_session sort_by event_weight
- P1: memory_write / memory_read 工具 + session_start 注入
- P3: boot_memory (system://boot 启动记忆注入)

## Key Files
- src/tools/session_summary.rs
- src/tools/hooks.rs
- src/workflow/journal.rs
- src/hooks/router.rs
- src/tools/search_session.rs
- src/tools/memory_write.rs (new)
- src/tools/memory_read.rs (new)
- src/main.rs, src/lib.rs, src/tools/mod.rs, src/params.rs
