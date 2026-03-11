# Writing Style: AI Context Injection

**Audience:** AI Agent modifying code
**Foundation:** OpenAI + Anthropic documentation principles
**Application:** Project conventions

## Core Insight

README.AI.md is AI's pre-loaded context before modifying code.
Every token MUST improve reasoning accuracy.

## Principle 1: Schema First

TypeScript/Rust interface > Table > List > Prose

```rust
struct Params {
    id: String,               // required
    mode: Mode,              // required, default: View
}
```

Then add semantic table:

| Param  | Type     | Required | Default | Description       |
| ------ | -------- | -------- | ------- | ----------------- |
| `id`   | `String` | ✓        | —       | Unique identifier |
| `mode` | `Mode`   | ✓        | `View`  | Operation mode    |

AI parses schemas faster than prose.

## Principle 2: Constraints as First-Class Citizens (RFC 2119)

Use MUST/MUST NOT/SHOULD keywords. No hedging.

✅ AI can execute:
```
Precondition: `input.id` MUST NOT be null.
IF null → throw `ValidationError("id is required")`
```

❌ AI cannot execute:
```
You should probably validate the input.
```

**Banned words:** probably, usually, might, sometimes, generally, often, typically

## Principle 3: Logic Formalization

Boolean expressions > Natural language.

❌ "If the user is logged in and has edit permission, or is an admin, show the save button"

✅ `showSave = (isLoggedIn AND hasEditPerm) OR isAdmin`

Enum values MUST document each value's behavior:

| Value  | Behavior             |
| ------ | -------------------- |
| `Edit` | Enable modifications |
| `View` | Read-only            |

## Principle 4: Self-Containment

Inline all types. AI may not have access to external files when reading README.

❌ "See User type in types.rs"

✅ Define inline:
```rust
struct User {
    id: String,
    name: String,
}
```

## Principle 5: Paired Examples

Show correct usage + common mistakes. AI calibrates through positive/negative pairs.

```rust
// ✅ Valid
handler(Params { id: "123".to_string(), mode: Edit });

// ❌ Missing required param
handler(Params { mode: Edit });
// → Runtime error: id required
```

## Principle 6: Token Efficiency

README consumes AI's context window. Density hierarchy: `Table` > `List` > `Paragraph`

**Remove:**
- Narrative openings: "Welcome to...", "This module is..."
- Vague modifiers: "powerful", "elegant", "flexible"
- Transition phrases: "As mentioned above..."

❌ Wastes tokens:
```
There are several types of errors that can occur.
Sometimes the network fails, and other times permissions might be wrong.
```

✅ High density:

| Error          | Condition    | Handling          |
| -------------- | ------------ | ----------------- |
| `NetworkError` | timeout > 5s | Retry 3×          |
| `AuthError`    | status 401   | Redirect `/login` |

## Principle 7: Language Neutrality

Framework-specific rules MUST be moved to lang-extensions/xx.md. Core principles remain generic.

## Quality Checklist

### AI-Readable
- [ ] Interface + parameter table?
- [ ] All constraints use MUST/MUST NOT?
- [ ] Logic as Boolean/Mermaid (not prose)?
- [ ] All types defined inline?
- [ ] Success + error examples paired?
- [ ] No hedging words?
- [ ] No narrative filler?
- [ ] Densest format used?

### General
- [ ] Single responsibility in one sentence?
- [ ] Data flow pattern declared?
- [ ] State strategy explicit?
- [ ] Inputs/outputs typed?
- [ ] Enum values documented with behavior?
- [ ] Each section understandable without source code?
