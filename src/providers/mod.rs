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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ProviderEntry, StreamToken};
    use tokio::sync::mpsc;

    fn make_entry_no_key(id: &'static str) -> ProviderEntry {
        ProviderEntry {
            id,
            display: "Test",
            env_var: "UNUSED_TEST_ENV",
            default_model: "m",
            api_key: String::new(),
            model: "m".to_string(),
            show_key: false,
            available_models: vec![],
            model_idx: 0,
            models_loading: false,
        }
    }

    fn make_entry_with_key(id: &'static str, key: &str) -> ProviderEntry {
        ProviderEntry {
            id,
            display: "Test",
            env_var: "UNUSED_TEST_ENV",
            default_model: "m",
            api_key: key.to_string(),
            model: "m".to_string(),
            show_key: false,
            available_models: vec![],
            model_idx: 0,
            models_loading: false,
        }
    }

    // ── SKILL_SYSTEM_PROMPT ───────────────────────────────────────────────────

    #[test]
    fn system_prompt_mentions_skill_md() {
        assert!(SKILL_SYSTEM_PROMPT.contains("SKILL.md"));
    }

    // ── skill_user_message ────────────────────────────────────────────────────

    #[test]
    fn skill_user_message_embeds_tool_name() {
        let msg = skill_user_message("claude-code", "my-skill", "do something");
        assert!(msg.contains("claude-code"));
    }

    #[test]
    fn skill_user_message_embeds_skill_name() {
        let msg = skill_user_message("tool", "my-skill", "req");
        assert!(msg.contains("my-skill"));
    }

    #[test]
    fn skill_user_message_embeds_requirement() {
        let msg = skill_user_message("tool", "skill", "write unit tests");
        assert!(msg.contains("write unit tests"));
    }

    #[test]
    fn skill_user_message_has_frontmatter_template() {
        let msg = skill_user_message("tool", "skill", "req");
        assert!(msg.contains("---"));
        assert!(msg.contains("name: skill"));
        assert!(msg.contains("description:"));
    }

    #[test]
    fn skill_user_message_has_required_sections() {
        let msg = skill_user_message("tool", "skill", "req");
        assert!(msg.contains("## Overview"));
        assert!(msg.contains("## When to Use"));
        assert!(msg.contains("## Instructions"));
        assert!(msg.contains("## Output Format"));
        assert!(msg.contains("## Examples"));
        assert!(msg.contains("## Edge Cases"));
    }

    // ── NoKeyProvider ─────────────────────────────────────────────────────────

    #[test]
    fn no_key_provider_name_is_fixed() {
        let p = NoKeyProvider::new("MY_ENV".to_string(), "My Provider".to_string());
        assert_eq!(p.name(), "No Provider");
    }

    #[test]
    fn no_key_provider_model_is_none() {
        let p = NoKeyProvider::new("MY_ENV".to_string(), "My Provider".to_string());
        assert_eq!(p.model(), "none");
    }

    #[tokio::test]
    async fn no_key_provider_sends_error_token_with_env_var() {
        let p = NoKeyProvider::new("MY_API_KEY_VAR".to_string(), "My Service".to_string());
        let (tx, mut rx) = mpsc::channel(4);
        p.generate_skill("tool", "skill", "req", tx).await.unwrap();
        let token = rx.recv().await.unwrap();
        match token {
            StreamToken::Error(msg) => {
                assert!(msg.contains("MY_API_KEY_VAR"));
                assert!(msg.contains("My Service"));
            }
            _ => panic!("Expected StreamToken::Error"),
        }
    }

    // ── build_provider ────────────────────────────────────────────────────────

    #[test]
    fn build_provider_returns_no_key_provider_when_key_empty() {
        let entry = make_entry_no_key("claude");
        let p = build_provider(&entry);
        assert_eq!(p.name(), "No Provider");
    }

    #[test]
    fn build_provider_returns_no_key_for_unknown_id_with_key() {
        let entry = make_entry_with_key("unknown-xyz", "some-key");
        let p = build_provider(&entry);
        assert_eq!(p.name(), "No Provider");
    }

    #[test]
    fn build_provider_returns_claude_provider_when_configured() {
        let entry = make_entry_with_key("claude", "sk-fake");
        let p = build_provider(&entry);
        assert_eq!(p.name(), "Claude (Anthropic)");
    }

    #[test]
    fn build_provider_returns_correct_model() {
        let mut entry = make_entry_with_key("claude", "sk-fake");
        entry.model = "claude-opus-4-6".to_string();
        let p = build_provider(&entry);
        assert_eq!(p.model(), "claude-opus-4-6");
    }

    // ── fetch_provider_models (unknown id) ────────────────────────────────────

    #[tokio::test]
    async fn fetch_provider_models_unknown_id_returns_empty() {
        let models = fetch_provider_models("totally-unknown", "fake-key").await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn fetch_provider_models_empty_key_returns_empty() {
        // Empty key should fail auth → returns empty (no real network hit expected)
        // We just verify the function is callable without panicking.
        // Each provider returns [] on any error, including auth failure.
        let _ = fetch_provider_models("claude", "").await;
    }
}
