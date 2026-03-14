#![allow(dead_code)]

pub mod claude;
pub mod gemini;
pub mod openai;
pub mod openrouter;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{ProviderEntry, StreamToken};

// ── Trait ─────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn generate_skill(
        &self,
        tool_name: &str,
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
            format!("Unknown provider '{}'", id),
        )),
    }
}
