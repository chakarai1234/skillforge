use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use super::AIProvider;
use crate::types::StreamToken;

// ── Model listing ─────────────────────────────────────────────────────────────
// GET https://api.openai.com/v1/models
// Auth: Authorization: Bearer $key
// Response: { "data": [{ "id": "gpt-4o", "owned_by": "openai" }] }
// We filter to only keep GPT/o-series models (exclude embedding, whisper, dall-e)

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Fetch available OpenAI models, keeping only chat-capable models.
pub async fn fetch_models(api_key: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.openai.com/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            r.json::<ModelsResponse>()
                .await
                .map(|m| {
                    let mut ids: Vec<String> = m
                        .data
                        .into_iter()
                        .map(|e| e.id)
                        // Keep only GPT and o-series chat models
                        .filter(|id| {
                            id.starts_with("gpt-")
                                || id.starts_with("o1")
                                || id.starts_with("o3")
                                || id.starts_with("o4")
                        })
                        .collect();
                    ids.sort();
                    ids.dedup();
                    ids
                })
                .unwrap_or_default()
        }
        _ => vec![],
    }
}

pub struct OpenAIProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    stream: bool,
    messages: Vec<OAIMessage>,
}

#[derive(Serialize)]
struct OAIMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAIChunk {
    choices: Vec<OAIChoice>,
}

#[derive(Deserialize)]
struct OAIChoice {
    delta: OAIDelta,
}

#[derive(Deserialize)]
struct OAIDelta {
    content: Option<String>,
}


#[async_trait]
impl AIProvider for OpenAIProvider {
    async fn generate_skill(
        &self,
        tool_name: &str,
        requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()> {
        let body = OpenAIRequest {
            model: self.model.clone(),
            stream: true,
            messages: vec![
                OAIMessage {
                    role: "system".to_string(),
                    content: super::SKILL_SYSTEM_PROMPT.to_string(),
                },
                OAIMessage {
                    role: "user".to_string(),
                    content: super::skill_user_message(tool_name, requirement),
                },
            ],
        };

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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
                401 => "Invalid API key — check your OPENAI_API_KEY".to_string(),
                429 => "Rate limited — please wait before retrying".to_string(),
                s => format!("Provider error (HTTP {})", s),
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
                            if let Ok(chunk) = serde_json::from_str::<OpenAIChunk>(data) {
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
        "OpenAI"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
