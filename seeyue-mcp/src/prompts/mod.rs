mod registry;
mod substitution;

use std::sync::Arc;

use rmcp::{
    ErrorData,
    handler::server::{
        prompt::PromptContext,
        router::prompt::{PromptRoute, PromptRouter},
    },
    service::MaybeSend,
};

use crate::error::ToolError;

pub use registry::SkillRegistry;

// ─── Prompt Router ──────────────────────────────────────────────────────────

pub fn build_prompt_router<S>(registry: Arc<SkillRegistry>) -> PromptRouter<S>
where
    S: MaybeSend + 'static,
{
    let mut router = PromptRouter::new();

    for skill in registry.entries() {
        if skill.disabled || !skill.available {
            continue;
        }

        let name = skill.name.clone();
        let prompt = skill.to_prompt();
        let registry = registry.clone();

        router.add_route(PromptRoute::new_dyn(prompt, move |context: PromptContext<'_, S>| {
            let registry = registry.clone();
            let name = name.clone();
            Box::pin(async move {
                registry
                    .get_prompt(&name, context.arguments.as_ref())
                    .map_err(to_prompt_error)
            })
        }));
    }

    router
}

fn to_prompt_error(err: ToolError) -> ErrorData {
    match err {
        ToolError::SkillNotFound { .. } => ErrorData::invalid_params(err.to_json(), None),
        _ => ErrorData::internal_error(err.to_json(), None),
    }
}
