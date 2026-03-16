use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use rmcp::model::{GetPromptResult, Prompt, PromptArgument, PromptMessage, PromptMessageRole};
use serde::Deserialize;

use crate::error::ToolError;

use super::substitution::apply_substitutions;

// ─── Skills Registry ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name:         String,
    pub title:        Option<String>,
    pub summary:      Option<String>,
    pub entry_path:   PathBuf,
    pub disabled:     bool,
    pub available:    bool,
    pub arguments:    Option<Vec<PromptArgument>>,
}

impl SkillEntry {
    pub fn to_prompt(&self) -> Prompt {
        let mut prompt = Prompt::new(self.name.clone(), self.summary.clone(), self.arguments.clone());
        if let Some(title) = &self.title {
            prompt = prompt.with_title(title.clone());
        }
        prompt
    }
}

pub struct SkillRegistry {
    skills: BTreeMap<String, SkillEntry>,
}

impl SkillRegistry {
    /// Load registry from workflow/skills.spec.yaml.
    /// Missing file → empty registry (non-blocking).
    pub fn load(workspace: &Path) -> Result<Self, String> {
        let spec_path = workspace.join("workflow").join("skills.spec.yaml");
        let content = match fs::read_to_string(&spec_path) {
            Ok(c) => c,
            Err(_) => return Ok(Self::load_empty(workspace)),
        };

        let spec: SkillsSpec = serde_yaml::from_str(&content)
            .map_err(|e| format!("Failed to parse skills.spec.yaml: {}", e))?;

        let mut skills = BTreeMap::new();

        for (name, skill) in spec.skills {
            let entry_path = workspace.join(&skill.entry);
            let available = entry_path.exists();
            let frontmatter = if available {
                read_frontmatter(&entry_path)
            } else {
                None
            };
            let argument_hint = frontmatter
                .as_ref()
                .and_then(|fm| fm.argument_hint.as_deref())
                .and_then(parse_argument_hint);

            let disabled = skill.policy.disable_model_invocation
                || frontmatter
                    .as_ref()
                    .and_then(|fm| fm.disable_model_invocation)
                    .unwrap_or(false);

            skills.insert(
                name.clone(),
                SkillEntry {
                    name,
                    title: skill.title,
                    summary: skill.summary,
                    entry_path,
                    disabled,
                    available,
                    arguments: argument_hint,
                },
            );
        }

        Ok(Self { skills })
    }

    pub fn load_empty(workspace: &Path) -> Self {
        let _ = workspace;
        Self { skills: BTreeMap::new() }
    }

    pub fn list_prompts(&self) -> Vec<Prompt> {
        let mut prompts: Vec<Prompt> = self.skills.values()
            .filter(|skill| skill.available && !skill.disabled)
            .map(SkillEntry::to_prompt)
            .collect();

        prompts.sort_by(|a, b| a.name.cmp(&b.name));
        prompts
    }

    pub fn entries(&self) -> impl Iterator<Item = &SkillEntry> {
        self.skills.values()
    }

    pub fn get_prompt(
        &self,
        name: &str,
        arguments: Option<&serde_json::Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, ToolError> {
        let skill = self.skills.get(name).ok_or_else(|| ToolError::SkillNotFound {
            name: name.to_string(),
            hint: "Skill not found in registry.".into(),
        })?;

        if skill.disabled || !skill.available {
            return Err(ToolError::SkillNotFound {
                name: name.to_string(),
                hint: "Skill is not available for prompts/get.".into(),
            });
        }

        let content = fs::read_to_string(&skill.entry_path).map_err(|e| ToolError::IoError {
            message: e.to_string(),
        })?;

        let resolved = apply_substitutions(&content, arguments);
        let mut result = GetPromptResult::new(vec![
            PromptMessage::new_text(PromptMessageRole::User, resolved),
        ]);

        if let Some(summary) = &skill.summary {
            result = result.with_description(summary.clone());
        }

        Ok(result)
    }
}

// ─── skills.spec.yaml structures ───────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
struct SkillsSpec {
    #[serde(default)]
    skills: BTreeMap<String, SkillSpec>,
}

#[derive(Debug, Deserialize)]
struct SkillSpec {
    title:   Option<String>,
    summary: Option<String>,
    entry:   String,
    #[serde(default)]
    policy:  SkillPolicy,
}

#[derive(Debug, Deserialize, Default)]
struct SkillPolicy {
    #[serde(default, rename = "disable_model_invocation")]
    disable_model_invocation: bool,
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    #[serde(rename = "argument-hint")]
    argument_hint: Option<String>,
    #[serde(rename = "disable-model-invocation")]
    disable_model_invocation: Option<bool>,
}

fn read_frontmatter(path: &Path) -> Option<SkillFrontmatter> {
    let content = fs::read_to_string(path).ok()?;
    let mut lines = content.lines();
    let first = lines.next()?.trim();
    if first != "---" {
        return None;
    }

    let mut yaml_lines = Vec::new();
    for line in lines.by_ref() {
        if line.trim() == "---" {
            break;
        }
        yaml_lines.push(line);
    }

    if yaml_lines.is_empty() {
        return None;
    }

    let yaml = yaml_lines.join("\n");
    serde_yaml::from_str::<SkillFrontmatter>(&yaml).ok()
}

fn parse_argument_hint(hint: &str) -> Option<Vec<PromptArgument>> {
    let trimmed = hint.trim();
    if trimmed.is_empty() {
        return None;
    }

    let core = trimmed
        .trim_start_matches('[')
        .trim_end_matches(']');

    let mut args = Vec::new();
    for part in core.split(',') {
        let name = part.trim().trim_matches('"').trim_matches('\'');
        if name.is_empty() {
            continue;
        }
        args.push(PromptArgument::new(name));
    }

    if args.is_empty() { None } else { Some(args) }
}
