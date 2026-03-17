---
name: sy-mcp
description: Use when invoking seeyue-mcp tools to enforce read-before-write, hook tool call order, checkpoint discipline, and session_context propagation.
allowed-tools:
  - Read
argument-hint: "[context]"
disable-model-invocation: false
---

# SY MCP Tool Protocol

## Overview

This skill governs correct usage of the `seeyue-mcp` MCP server tools within the
`sy-*` workflow stack.

- MCP tools provide deterministic workspace I/O and policy state.
- Hook tools (`sy_*`) bridge the MCP layer with the hook binary (`sy-hook.exe`).
- Correct call order and evidence capture are enforced here; the hook binary
  enforces hard guards independently.

## Trigger

Use when:
- any MCP tool from `seeyue-mcp` is about to be called
- selecting between read / write / edit tools
- invoking hook policy tools (`sy_pretool_bash`, `sy_pretool_write`, etc.)
- creating checkpoints or rewinding workspace state
- reading session state or workflow resources

## When NOT to Use

- pure non-MCP bash commands with no file I/O
- read-only exploration with no write intent

---

## Tool Priority Tiers

| Tier | Tools | Rule |
|------|-------|------|
| P0 | `read_file`, `write`, `edit`, `multi_edit`, `rewind` | Always read before write. Required for all file operations. |
| P1 | `sy_pretool_bash`, `sy_pretool_write`, `sy_posttool_write`, `sy_stop`, `sy_create_checkpoint`, `sy_advance_node` | Hook policy tools. Call at hook decision points. |
| P2 | `file_outline`, `find_definition`, `find_references`, `verify_syntax`, `type_check`, `lint_file`, `run_test`, `run_command`, `session_summary` | Analysis tools. Call when context requires. |
| P3 | `dependency_graph`, `multi_file_edit`, `create_file_tree`, `package_info`, `symbol_rename_preview`, `diff_since_checkpoint` | Complex operations. Call only when explicitly needed. |

---

## P0: Read-Before-Write Protocol (MUST)

```text
RULE: MUST call read_file before any write / edit / multi_edit on a file.
VIOLATION: write or edit without prior read → FILE_NOT_READ error from MCP server.
```

Call sequence:
```
1. read_file(file_path)          ← always first
2. [inspect content]
3. edit(file_path, old, new)     ← only after read
   OR write(file_path, content)  ← only if creating new or full overwrite
```

For `multi_edit`: same rule — read_file first, then multi_edit.

For `multi_file_edit`: read each target file before passing edits.

`old_string` in edit MUST be copied verbatim from read output.
Tab characters appear as `\t` in read output — preserve exactly.

---

## P1: Hook Tool Call Protocol

Hook tools mirror the `sy-hook.exe` event lifecycle. Call them at the corresponding
decision points during workflow execution.

### sy_pretool_bash

Call before any significant bash command.

```text
Purpose: classify command risk + enforce loop budget guard
Input:   command string
Output:  verdict {allow|block|notify} + session_context
Action:
  IF verdict == block  → STOP, surface reason, do not run the command
  IF verdict == notify → run command, log notification
  IF verdict == allow  → proceed
```

Read `session_context` from verdict response:
- `budget_exceeded: true` → STOP execution, surface to user
- `restore_pending: true` → resolve recovery before proceeding

### sy_pretool_write

Call before any write / edit operation on production files.

```text
Purpose: enforce TDD gate, secret guard, protected-file guard
Input:   file_path
Output:  verdict + session_context
Action:
  IF verdict == block → STOP write, surface reason
  Read tdd_state from session_context:
    IF tdd_state == "red_required" → must run execute-test first
```

### sy_posttool_write

Call after every successful write / edit.

```text
Purpose: capture write evidence into journal, detect scope drift
Input:   path, tool ("Write"|"Edit"), lines_changed (optional)
Output:  evidence record written to .ai/workflow/journal.jsonl
```

Required fields:
- `path`: file that was written
- `tool`: "Write" or "Edit"
- `lines_changed`: line count delta if known (improves audit trail)

### sy_stop

Call before completing a response that claims work is done.

```text
Purpose: phase-aware completion checkpoint gate
Input:   reason (optional)
Output:  verdict + pending_approvals count
Action:
  IF verdict == block → do NOT emit completion, surface pending items
  IF pending_approvals > 0 → list them before stopping
```

### sy_create_checkpoint

Call at phase boundaries and before risky operations.

```text
Purpose: snapshot workspace state for rollback
Input:   label (required), files (optional — list of files to snapshot)
Output:  checkpoint_id stored in session recovery state
```

Checkpoint naming convention:
```
"before-{phase}-{node_id}"     # before executing a risky node
"after-{phase}-{node_id}"      # after verified node completion
"pre-refactor-{description}"   # before large-scale changes
```

Checkpoint MUST precede:
- any schema / public API / security boundary change
- any multi-file edit across 3+ files
- phase transitions (plan → execute → review)

### sy_advance_node

Call when a workflow node changes state.

```text
Purpose: advance TDD state machine + update session.yaml node state
Input:   node_id (required), status, state, tdd_required, name, target
Output:  updated session state + budget check
Action:
  IF budget_exceeded in response → STOP, surface to user
```

TDD state machine:
```
idle → red_required → red_confirmed → green_confirmed → verified
```

Call sequence for a tdd_required node:
```
1. sy_advance_node(node_id, state="red_required")   # before execute-test
2. [run test → confirm RED]
3. sy_advance_node(node_id, state="red_confirmed")  # after RED confirmed
4. [implement node]
5. sy_advance_node(node_id, state="green_confirmed") # after tests pass
6. [run verify]
7. sy_advance_node(node_id, status="complete")       # after verify passes
```

---

## Session Context Propagation

Every `sy_pretool_bash`, `sy_pretool_write`, and `sy_stop` verdict response
carries a `session_context` field. MUST read these fields:

| Field | Type | Action Required |
|-------|------|-----------------|
| `budget_exceeded` | bool | true → STOP, do not continue execution |
| `restore_pending` | bool | true → resolve recovery before any write |
| `tdd_state` | string | drive execute-node/test/verify routing |
| `pending_approvals` | int | > 0 → list approvals before stopping |
| `last_checkpoint_id` | string | track for rewind reference |
| `run_id` | string | correlate journal events |

---

## P2: Analysis Tool Usage

### file_outline

Call before editing a file to understand its symbol structure.

```text
When: before editing a file with unknown structure
Do NOT: read the full file just to find a function — use file_outline first
```

### verify_syntax + type_check

Call after every write to a source file.

```text
Order: verify_syntax → type_check → lint_file
If verify_syntax fails → do NOT proceed to type_check or commit
```

### run_test

Call to run project tests. Maps to `execute-test` in the 3-split operation model.

```text
When: after execute-node (TDD green phase)
Then: sy_advance_node(state="green_confirmed") after test passes
```

### session_summary

Call to read current workflow session state without touching session.yaml directly.

```text
Prefer this over reading .ai/workflow/session.yaml directly.
Outputs: phase, node, tdd_state, loop_budget, recovery_status
```

---

## P3: Complex Operation Guards

### dependency_graph

```text
Timeout: allow 30s (large files need static import graph traversal)
When: understanding module coupling before refactor
```

### multi_file_edit

```text
Precondition: read each target file first
Checkpoint: sy_create_checkpoint before calling
Max files: prefer ≤ 5 per call for atomicity
```

### rewind

```text
When: undoing last N write operations
Requires: checkpoint created before the writes being undone
Do NOT: use rewind as a substitute for rollback_boundary planning
```

---

## Workflow Integration Map

Shows when to call which MCP tools in the `sy-*` execution flow.

| Workflow Stage | MCP Tools to Call |
|---------------|-------------------|
| Session start | `sy_session_start`, `session_summary` |
| Before execute-node | `sy_pretool_write(path)`, `sy_create_checkpoint(label)` |
| During execute-node | `read_file` → `edit`/`write` → `sy_posttool_write` |
| After each write | `verify_syntax`, then `sy_posttool_write` |
| execute-test (RED) | `run_test` → `sy_advance_node(state=red_confirmed)` |
| execute-test (GREEN) | `run_test` → `sy_advance_node(state=green_confirmed)` |
| execute-verify | `type_check`, `lint_file`, `run_test` → `sy_advance_node(status=complete)` |
| Phase transition | `sy_create_checkpoint` → `sy_advance_node` |
| Before stop | `sy_stop` |
| Bash command | `sy_pretool_bash(command)` before running |

---

## Iron Rules (MUST)

```text
MUST call read_file before write/edit/multi_edit.
MUST call sy_pretool_write before writing production source files.
MUST call sy_posttool_write after every successful write/edit.
MUST call sy_create_checkpoint before schema/API/security boundary changes.
MUST call sy_stop before claiming task complete.
MUST check session_context.budget_exceeded after every sy_pretool_bash call.
MUST check session_context.restore_pending before any write if recovery is active.
MUST NOT call write on a file that returned FILE_NOT_READ error — read first.
MUST NOT skip verify_syntax after writing source code.
```

---

## Hook Binary Reference

The hook binary `sy-hook.exe` runs these events independently of MCP tool calls:

| Hook Event | Binary Invocation | Mirrors MCP Tool |
|-----------|-------------------|------------------|
| SessionStart | `sy-hook.exe SessionStart` | `sy_session_start` |
| UserPromptSubmit | `sy-hook.exe UserPromptSubmit` | — |
| PreToolUse:Bash | `sy-hook.exe PreToolUse:Bash` | `sy_pretool_bash` |
| PreToolUse:Write|Edit | `sy-hook.exe PreToolUse:Write|Edit` | `sy_pretool_write` |
| PostToolUse:Write|Edit | `sy-hook.exe PostToolUse:Write|Edit` | `sy_posttool_write` |
| PostToolUse:Bash | `sy-hook.exe PostToolUse:Bash` | — |
| Stop | `sy-hook.exe Stop` | `sy_stop` |

Key distinction:
- Hook binary fires **deterministically** on every matching tool call.
- MCP hook tools (`sy_pretool_*`) are called **explicitly** by the model for
  pre-flight policy checks.
- Both layers are complementary. The hook binary is the last line of defense;
  MCP tools give the model policy context before acting.

---

## Red Flags

- "直接 write，反正内容我知道是对的" — read-before-write skipped → FILE_NOT_READ
- "old_string 大概是这样" — verbatim copy required, tab/space matters
- "先跑完再 checkpoint" — checkpoint must precede risky operation, not follow
- "budget 超了但只差一点" — budget_exceeded=true is a hard stop, not advisory
- "session_context 不重要" — restore_pending=true means writes are blocked
- "verify_syntax 之后再说" — syntax must pass before posttool or type_check

## Related Skills

- `sy-constraints`
- `sy-constraints/execution`
- `sy-constraints/testing`
- `sy-executing-plans`
- `sy-workflow`