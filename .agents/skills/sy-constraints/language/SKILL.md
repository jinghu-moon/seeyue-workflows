---
name: sy-constraints/language
description: Use when defining, editing, or auditing workflow rules so language partitions and RFC keyword discipline stay consistent.
allowed-tools:
  - Read
argument-hint: [context]
disable-model-invocation: false
---

# Constraints: Language

## Overview

This skill enforces language partition boundaries for rule text, user output, and executable artifacts.

## Trigger

Use when:
- writing or editing any constraint/prompt/spec block
- reviewing whether a skill violates language partition rules

## Iron Rule

```text
RULES/PERSONA/RFC2119 MUST be English.
USER_OUTPUT MUST be zh-CN.
CODE/CMD/PATH MUST stay verbatim.
```

## Protocol

1. RULES / PERSONA / RFC2119 blocks MUST be written in English.
2. USER_OUTPUT blocks MUST be written in zh-CN.
3. CODE / CMD / PATH tokens MUST remain verbatim.
4. RED_FLAGS SHOULD be written in zh-CN when user language is zh-CN.
5. RFC keywords MUST remain uppercase: MUST / MUST NOT / SHOULD / SHOULD NOT / MAY.

Scope tags:

```text
[RULE]     -> English only
[PERSONA]  -> English only
[OUTPUT]   -> zh-CN only
[CODE]     -> verbatim
[RED_FLAG] -> zh-CN preferred
```

MUST NOT with alternatives:
- MUST NOT translate code/command/path tokens into natural language.
  - Alternative: keep token verbatim and add explanation in adjacent zh-CN sentence.
- MUST NOT replace RFC keywords with localized variants inside normative blocks.
  - Alternative: keep `MUST/MUST NOT/SHOULD/SHOULD NOT/MAY` in English and add a separate plain-language explanation if needed.
- MUST NOT mix `[RULE]` and `[OUTPUT]` content in one sentence.
  - Alternative: split into two lines, one normative English rule and one user-facing zh-CN output line.

## Rationalization Table — All Invalid

| Excuse | Reality |
|---|---|
| "把 MUST 换成中文更自然" | RFC semantics become ambiguous after translation. |
| "命令翻译一下更友好" | Translated commands are not executable and increase error risk. |
| "规则和输出写在一段更省 tokens" | Mixed language blocks degrade parseability and enforcement. |

## Record Format

```text
LanguageCheck:
  rule_blocks: pass|fail
  output_blocks: pass|fail
  code_tokens_verbatim: pass|fail
  rfc_keywords_uppercase: pass|fail
```

## Red Flags

- "先给我英文提示，中文我自己看"
- "把 git 命令翻译成中文解释后再执行"
- "MUST 改成必须，语气更自然"

## When NOT to use

- Pure code execution where no rule/prompt text is being authored.

## Related Skills

- `sy-constraints`
- `sy-constraints/truth`
- `sy-constraints/execution`
