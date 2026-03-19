// tests/compat_schema.rs
// TDD tests for M-N4: ClientType schema sanitization.
// Run: cargo test --test compat_schema

use seeyue_mcp::tools::compat::{ClientType, sanitize_for_client};
use serde_json::json;

// M-N4 test 1: Claude — schema unchanged
#[test]
fn test_claude_schema_unchanged() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema",
        "additionalProperties": false,
        "properties": { "path": { "type": "string" } }
    });
    let result = sanitize_for_client(schema.clone(), ClientType::Claude);
    assert_eq!(result, schema);
}

// M-N4 test 2: OpenAI — removes additionalProperties and $schema
#[test]
fn test_openai_removes_additional_properties() {
    let schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema",
        "additionalProperties": false,
        "properties": { "path": { "type": "string" } }
    });
    let result = sanitize_for_client(schema, ClientType::OpenAI);
    assert!(result.get("additionalProperties").is_none(),
        "OpenAI: additionalProperties should be removed");
    assert!(result.get("$schema").is_none(),
        "OpenAI: $schema should be removed");
    assert!(result.get("properties").is_some(),
        "OpenAI: properties should remain");
}

// M-N4 test 3: Gemini — flattens anyOf with nullable
#[test]
fn test_gemini_flattens_any_of() {
    let schema = json!({
        "anyOf": [
            { "type": "string" },
            { "type": "null" }
        ]
    });
    let result = sanitize_for_client(schema, ClientType::Gemini);
    // Gemini: anyOf [T, null] → { type: T, nullable: true }
    assert!(result.get("anyOf").is_none() || result.get("nullable").is_some()
        || result.get("type").is_some(),
        "Gemini: should simplify anyOf, got: {:?}", result);
}

// M-N4 test 4: Unknown — applies both OpenAI and Gemini transforms
#[test]
fn test_unknown_applies_conservative_transforms() {
    let schema = json!({
        "$schema": "x",
        "additionalProperties": false,
        "properties": { "x": { "type": "string" } }
    });
    let result = sanitize_for_client(schema, ClientType::Unknown);
    assert!(result.get("additionalProperties").is_none());
    assert!(result.get("$schema").is_none());
}

// M-N4 test 5: original schema not mutated (clone passed)
#[test]
fn test_original_schema_not_mutated() {
    let schema = json!({ "additionalProperties": false });
    let original = schema.clone();
    sanitize_for_client(schema, ClientType::OpenAI);
    // original is not passed by reference, so this is a compile-time guarantee
    // but we verify the clone matches what we constructed
    assert_eq!(original["additionalProperties"], false);
}
