// OpenRouter — OpenAI-compatible chat completions with SSE streaming
// Endpoint: POST https://openrouter.ai/api/v1/chat/completions
// Auth: Authorization: Bearer {OPENROUTER_API_KEY}
// Docs: https://openrouter.ai/docs/api/reference/streaming
//
// OpenRouter is a drop-in OpenAI-compatible API — same request/response schema,
// just a different base URL. Additional optional headers for rankings:
//   HTTP-Referer: <your-site>
//   X-Title: <your-app-name>

use anyhow::Result;
use async_trait::async_trait;

// ── Model listing ─────────────────────────────────────────────────────────────
// GET https://openrouter.ai/api/v1/models
// Auth: Authorization: Bearer $key
// Response: { "data": [{ "id": "openai/gpt-4o", "name": "GPT-4o", ... }] }
// OpenRouter hosts 400+ models — we keep only top chat models from major providers.

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct OrModelsResponse {
    data: Vec<OrModelEntry>,
}

#[allow(dead_code)]
#[derive(serde::Deserialize)]
struct OrModelEntry {
    id: String,
}

/// Fetch available OpenRouter models, filtered to major provider chat models.
pub async fn fetch_models(api_key: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://openrouter.ai/api/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            r.json::<OrModelsResponse>()
                .await
                .map(|m| {
                    let mut ids: Vec<String> = m
                        .data
                        .into_iter()
                        .map(|e| e.id)
                        // Keep models from the main providers
                        .filter(|id| {
                            id.starts_with("openai/")
                                || id.starts_with("anthropic/")
                                || id.starts_with("google/")
                                || id.starts_with("meta-llama/")
                                || id.starts_with("mistralai/")
                                || id.starts_with("deepseek/")
                                || id.starts_with("qwen/")
                        })
                        .take(60) // cap at 60 to keep navigation manageable
                        .collect();
                    ids.sort();
                    ids
                })
                .unwrap_or_default()
        }
        _ => vec![],
    }
}
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::AIProvider;
use crate::types::StreamToken;

pub struct OpenRouterProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl OpenRouterProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

// ── Request / Response types (identical schema to OpenAI) ─────────────────────

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    stream: bool,
    messages: Vec<ChatMessage>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    delta: ChatDelta,
}

#[derive(Deserialize)]
struct ChatDelta {
    content: Option<String>,
}

// ── Provider implementation ────────────────────────────────────────────────────

#[async_trait]
impl AIProvider for OpenRouterProvider {
    async fn generate_skill(
        &self,
        tool_name: &str,
        requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()> {
        let body = ChatRequest {
            model: self.model.clone(),
            stream: true,
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: super::SKILL_SYSTEM_PROMPT.to_string(),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: super::skill_user_message(tool_name, requirement),
                },
            ],
        };

        let response = self
            .client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            // Optional headers for OpenRouter ranking/analytics
            .header("HTTP-Referer", "https://github.com/skillforge-cli")
            .header("X-Title", "SkillForge CLI")
            .json(&body)
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                let _ = tx
                    .send(StreamToken::Error(format!("Connection error: {}", e)))
                    .await;
                return Ok(());
            }
        };

        if !response.status().is_success() {
            let msg = match response.status().as_u16() {
                401 => "Invalid API key — check your OPENROUTER_API_KEY".to_string(),
                402 => "Insufficient credits — top up your OpenRouter account".to_string(),
                429 => "Rate limited — please wait before retrying".to_string(),
                s => format!("OpenRouter error (HTTP {})", s),
            };
            let _ = tx.send(StreamToken::Error(msg)).await;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buf = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx
                        .send(StreamToken::Error(format!("Stream error: {}", e)))
                        .await;
                    return Ok(());
                }
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));

            loop {
                match buf.find('\n') {
                    None => break,
                    Some(pos) => {
                        let line = buf[..pos].trim_end_matches('\r').to_string();
                        buf = buf[pos + 1..].to_string();

                        if let Some(data) = line.strip_prefix("data: ") {
                            if data.trim() == "[DONE]" {
                                let _ = tx.send(StreamToken::Done).await;
                                return Ok(());
                            }
                            if let Ok(chunk) = serde_json::from_str::<ChatChunk>(data) {
                                if let Some(choice) = chunk.choices.first() {
                                    if let Some(content) = &choice.delta.content {
                                        if !content.is_empty()
                                            && tx
                                                .send(StreamToken::Token(content.clone()))
                                                .await
                                                .is_err()
                                        {
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let _ = tx.send(StreamToken::Done).await;
        Ok(())
    }

    fn name(&self) -> &str {
        "OpenRouter"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
