use serde_json::{Map, Value};

// ─── 占位符替换 ─────────────────────────────────────────────────────────────

pub fn apply_substitutions(
    content: &str,
    arguments: Option<&Map<String, Value>>,
) -> String {
    let mut resolved = content.to_string();

    match arguments {
        Some(args) => {
            // $ARGUMENTS：优先 ARGUMENTS，其次 task
            let full_args = args
                .get("ARGUMENTS")
                .or_else(|| args.get("task"))
                .map(value_to_string)
                .unwrap_or_default();
            resolved = resolved.replace("$ARGUMENTS", &full_args);

            // $CUSTOM_INSTRUCTIONS：支持注入扩展约束
            let custom = args
                .get("custom_instructions")
                .or_else(|| args.get("CUSTOM_INSTRUCTIONS"))
                .map(value_to_string)
                .unwrap_or_default();
            resolved = resolved.replace("$CUSTOM_INSTRUCTIONS", &custom);

            // $0/$1...：按参数顺序替换
            for (idx, (_k, v)) in args.iter().enumerate() {
                let key = format!("${}", idx);
                resolved = resolved.replace(&key, &value_to_string(v));
            }
        }
        None => {
            resolved = resolved.replace("$ARGUMENTS", "");
            resolved = resolved.replace("$CUSTOM_INSTRUCTIONS", "");
        }
    }

    resolved
}

fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}
