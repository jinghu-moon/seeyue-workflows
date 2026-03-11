# Lang Extension: TypeScript

Load when: `tsconfig.json` detected.

## Additional Interface Fields

```json
{
  "exported_types": ["interface Name { fields }", "type Name = ..."],
  "exported_functions": ["function name(params): ReturnType"],
  "exported_classes": ["class Name { methods }"],
  "generics": ["<T extends Constraint>"],
  "module_exports": ["export default", "named exports"]
}
```

## TypeScript-Specific Constraints

- MUST document all exported interfaces and type aliases
- MUST use strict mode (`strict: true` in tsconfig)
- MUST NOT use `any` — use `unknown` with type narrowing
- MUST document generic constraints and their purpose

## Type Narrowing Patterns

```typescript
// MUST use type guards instead of `as` casting
function isUser(value: unknown): value is User {
  return typeof value === 'object' && value !== null && 'id' in value;
}
```

## Module Export Conventions

| Pattern | When |
|---------|------|
| `export default` | Single primary export per file |
| `export { named }` | Multiple exports |
| `export type { T }` | Type-only exports (no runtime) |
| `export * from` | Re-exports from barrel files |

## Generic Constraints

Document each generic parameter:

```typescript
// T: The data type stored in the cache
// K: Cache key, MUST be string-compatible
interface Cache<T, K extends string = string> {
  get(key: K): T | undefined;
  set(key: K, value: T): void;
}
```

## Error Handling Pattern

```typescript
// MUST use discriminated unions for error types
type Result<T, E = Error> =
  | { ok: true; value: T }
  | { ok: false; error: E };
```

## Quality Checklist (TypeScript-Specific)

- [ ] All exported types documented?
- [ ] Generic constraints explained?
- [ ] No `any` usage (use `unknown`)?
- [ ] Type guards for narrowing?
- [ ] Discriminated unions for errors?
