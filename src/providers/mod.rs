#![allow(dead_code)]

pub mod claude;
pub mod gemini;
pub mod openai;
pub mod openrouter;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{ProviderEntry, StreamToken};

// ── Shared skill prompt ───────────────────────────────────────────────────────

/// System prompt used by every provider when generating a SKILL.md.
pub const SKILL_SYSTEM_PROMPT: &str = "\
You are an expert at writing SKILL.md files for AI skill systems.\n\
A SKILL.md is a markdown file with YAML frontmatter that instructs an AI on how to perform a specialised task.\n\
Research the tool thoroughly using any available web search capabilities before writing.\n\n\
Output ONLY the raw SKILL.md content — no preamble, no explanation, no code fences.";

/// Build the user message that embeds the tool name, requirement, and the
/// exact SKILL.md template the model must follow.
pub fn skill_user_message(tool_name: &str, skill_name: &str, requirement: &str) -> String {
    format!(
        "Create a SKILL.md for: **{tool_name}**\n\
Requirement: {requirement}\n\n\
Follow this structure exactly:\n\n\
---\n\
name: {skill_name}\n\
description: >\n\
  <When to trigger this skill — be specific and \"pushy\" so the AI uses it proactively.\n\
  Include keywords, user phrases, and contexts. Also describe what it does.>\n\
---\n\n\
# <Skill Title>\n\n\
## Overview\n\
<What this skill does and why it exists>\n\n\
## When to Use\n\
<Explicit trigger conditions>\n\n\
## Instructions\n\
<Step-by-step instructions the AI should follow>\n\n\
## Output Format\n\
<Expected output structure>\n\n\
## Examples\n\
<1–2 concrete input/output examples>\n\n\
## Edge Cases\n\
<Known gotchas or failure modes>\n\n\
Rules:\n\
- Keep SKILL.md under 500 lines\n\
- Put ALL triggering logic in the frontmatter description, not the body\n\
- Be explicit about tools, dependencies, or APIs needed\n\
- Use progressive disclosure: SKILL.md for overview, reference files for deep detail\n\
- Make instructions deterministic — avoid ambiguity"
    )
}

// ── Trait ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn generate_skill(
        &self,
        tool_name: &str,
        skill_name: &str,
        requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()>;

    fn name(&self) -> &str;
    fn model(&self) -> &str;
}

// ── No-key fallback ───────────────────────────────────────────────────────────

pub struct NoKeyProvider {
    env_var: String,
    provider_name: String,
}

impl NoKeyProvider {
    pub fn new(env_var: String, provider_name: String) -> Self {
        Self {
            env_var,
            provider_name,
        }
    }
}

#[async_trait]
impl AIProvider for NoKeyProvider {
    async fn generate_skill(
        &self,
        _tool_name: &str,
        _skill_name: &str,
        _requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()> {
        let msg = format!(
            "No API key configured for {}. Go to the Providers tab (press 2) and enter your {}.",
            self.provider_name, self.env_var
        );
        let _ = tx.send(StreamToken::Error(msg)).await;
        Ok(())
    }

    fn name(&self) -> &str {
        "No Provider"
    }

    fn model(&self) -> &str {
        "none"
    }
}

// ── Model listing dispatcher ──────────────────────────────────────────────────

/// Fetch the list of available models for the given provider.
/// Each provider has its own API endpoint and response format.
pub async fn fetch_provider_models(provider_id: &str, api_key: &str) -> Vec<String> {
    match provider_id {
        "claude" => claude::fetch_models(api_key).await,
        "openai" => openai::fetch_models(api_key).await,
        "gemini" => gemini::fetch_models(api_key).await,
        "openrouter" => openrouter::fetch_models(api_key).await,
        _ => vec![],
    }
}

// ── Factory ───────────────────────────────────────────────────────────────────

/// Build a provider from a live ProviderEntry (with user-entered key + model).
pub fn build_provider(entry: &ProviderEntry) -> Box<dyn AIProvider> {
    if entry.api_key.is_empty() {
        return Box::new(NoKeyProvider::new(
            entry.env_var.to_string(),
            entry.display.to_string(),
        ));
    }

    match entry.id {
        "claude" => Box::new(claude::ClaudeProvider::new(
            entry.api_key.clone(),
            entry.model.clone(),
            None,
        )),
        "openai" => Box::new(openai::OpenAIProvider::new(
            entry.api_key.clone(),
            entry.model.clone(),
            None,
        )),
        "gemini" => Box::new(gemini::GeminiProvider::new(
            entry.api_key.clone(),
            entry.model.clone(),
        )),
        "openrouter" => Box::new(openrouter::OpenRouterProvider::new(
            entry.api_key.clone(),
            entry.model.clone(),
        )),
        id => Box::new(NoKeyProvider::new(
            entry.env_var.to_string(),
            format!("Unknown provider '{id}'"),
        )),
    }
}
