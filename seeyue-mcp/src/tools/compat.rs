// src/tools/compat.rs
// Client compatibility layer (M-N4).
// Sanitizes JSON schema for different MCP client types.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientType {
    Claude,
    OpenAI,
    Gemini,
    Unknown,
}

/// Sanitize a JSON schema for the given client type.
/// Takes ownership of the schema (caller passes a clone if needed).
pub fn sanitize_for_client(schema: Value, client: ClientType) -> Value {
    match client {
        ClientType::Claude  => schema,
        ClientType::OpenAI  => remove_openai_incompatible(schema),
        ClientType::Gemini  => flatten_gemini_nullable(schema),
        ClientType::Unknown => {
            let s = remove_openai_incompatible(schema);
            flatten_gemini_nullable(s)
        }
    }
}

/// Remove fields not supported by OpenAI: additionalProperties, $schema, const.
fn remove_openai_incompatible(mut schema: Value) -> Value {
    if let Some(obj) = schema.as_object_mut() {
        obj.remove("additionalProperties");
        obj.remove("$schema");
        obj.remove("const");

        // Recurse into properties
        if let Some(Value::Object(props)) = obj.get_mut("properties") {
            let keys: Vec<String> = props.keys().cloned().collect();
            for k in keys {
                if let Some(v) = props.get(&k).cloned() {
                    props.insert(k, remove_openai_incompatible(v));
                }
            }
        }
    }
    schema
}

/// Flatten anyOf [T, null] → { type: T, nullable: true } for Gemini.
fn flatten_gemini_nullable(schema: Value) -> Value {
    if let Value::Object(ref obj) = schema {
        if let Some(Value::Array(variants)) = obj.get("anyOf") {
            let non_null: Vec<&Value> = variants.iter()
                .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("null"))
                .collect();
            let has_null = variants.iter()
                .any(|v| v.get("type").and_then(|t| t.as_str()) == Some("null"));

            if has_null && non_null.len() == 1 {
                let mut new_obj = non_null[0].clone();
                if let Some(o) = new_obj.as_object_mut() {
                    o.insert("nullable".to_string(), Value::Bool(true));
                }
                return new_obj;
            }
        }
    }
    schema
}
