# MCP Memory Enhancements Plan

Date: 2026-03-17 23:44:32
Branch: main

## Context

Based on Memory-Palace and nocturne_memory reference projects.
Extending seeyue-mcp with 8 new capabilities across 4 priority tiers.

## Steps

- [ ] Step 1: memory_delete tool (new file: tools/memory_delete.rs)
- [ ] Step 2: memory_list tool (new file: tools/memory_list.rs)
- [ ] Step 3: search_session since/until time filter (modify tools/search_session.rs)
- [ ] Step 4: session_summary event_counts field (modify tools/session_summary.rs)
- [ ] Step 5: memory_write append mode (modify tools/memory_write.rs)
- [ ] Step 6: tdd_evidence_summary tool (new file: tools/tdd_evidence.rs)
- [ ] Step 7: checkpoint_list tool (new file: tools/checkpoint_list.rs)
- [ ] Step 8: sy_session_end hook (modify tools/hooks.rs)
- [ ] Step 9: memory://index MCP resource (modify resources/)

## Registration per step
- tools/mod.rs: add pub mod
- params.rs: add param structs
- main.rs: register tool handlers
- tests/: add test file per new tool
