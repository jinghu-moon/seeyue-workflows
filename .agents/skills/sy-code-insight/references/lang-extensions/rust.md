# Lang Extension: Rust

Load when: `Cargo.toml` detected.

## Additional Interface Fields

```json
{
  "pub_functions": ["fn name(params) -> ReturnType"],
  "pub_structs": ["struct Name { fields }"],
  "pub_enums": ["enum Name { Variants }"],
  "traits_impl": ["impl Trait for Type"],
  "feature_gates": ["#[cfg(feature = \"name\")]"]
}
```

## Rust-Specific Constraints

- MUST document `unsafe` blocks with safety invariants
- MUST list all `#[cfg(feature)]` gates and their effects
- MUST document lifetime relationships for public APIs
- MUST NOT ignore `Result` — handle or propagate with `?`

## Feature Gates

Document each feature and its effect:

| Feature | Dependencies | Effect |
|---------|-------------|--------|
| `default` | — | Base functionality |
| `serde` | `dep:serde` | Serialization support |

## Module Visibility

- `pub` → public API, MUST document
- `pub(crate)` → crate-internal, document if complex
- `pub(super)` → parent module only
- Private → skip in interface schema

## Error Handling Pattern

```rust
// MUST use thiserror or anyhow for error types
#[derive(thiserror::Error, Debug)]
enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io error")]
    Io(#[from] std::io::Error),
}
```

## FFI / Unsafe

IF `unsafe` or FFI detected:
- MUST document safety invariants
- MUST list all `extern` functions
- MUST note platform requirements (e.g., Windows-only)

## Quality Checklist (Rust-Specific)

- [ ] All `pub` items documented?
- [ ] Feature gates listed with effects?
- [ ] `unsafe` blocks have safety comments?
- [ ] Error types enumerated?
- [ ] Lifetime constraints documented?
