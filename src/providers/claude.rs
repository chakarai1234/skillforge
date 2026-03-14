use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::types::StreamToken;
use super::AIProvider;

// ── Model listing ─────────────────────────────────────────────────────────────
// GET https://api.anthropic.com/v1/models
// Auth: x-api-key + anthropic-version header
// Response: { "data": [{ "id": "claude-sonnet-4-20250514", "display_name": "..." }] }

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelEntry>,
}

#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

/// Fetch available Claude models from the Anthropic API.
pub async fn fetch_models(api_key: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.anthropic.com/v1/models")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            r.json::<ModelsResponse>()
                .await
                .map(|m| m.data.into_iter().map(|e| e.id).collect())
                .unwrap_or_default()
        }
        _ => vec![],
    }
}

pub struct ClaudeProvider {
    api_key: String,
    model: String,
    base_url: String,
    client: reqwest::Client,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            api_key,
            model,
            base_url: base_url
                .unwrap_or_else(|| "https://api.anthropic.com".to_string()),
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    stream: bool,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct ClaudeEvent {
    #[serde(rename = "type")]
    event_type: String,
    delta: Option<ClaudeDelta>,
    error: Option<ClaudeErrorBody>,
}

#[derive(Deserialize, Debug)]
struct ClaudeDelta {
    #[serde(rename = "type")]
    delta_type: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize, Debug)]
struct ClaudeErrorBody {
    message: String,
}

const SYSTEM_PROMPT: &str = "You are an expert at writing AI skill definitions for CLI tools.\
\nA skill is a markdown document that teaches an AI assistant how to help with a specific CLI tool.\
\nIt must include:\
\n1. A brief description of the tool\
\n2. Common workflows and commands\
\n3. Best practices and gotchas\
\n4. Example prompts that work well with the tool\
\n\nOutput ONLY the markdown content. No preamble, no explanation outside the markdown.";

#[async_trait]
impl AIProvider for ClaudeProvider {
    async fn generate_skill(
        &self,
        tool_name: &str,
        requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()> {
        let body = ClaudeRequest {
            model: self.model.clone(),
            max_tokens: 2048,
            stream: true,
            system: SYSTEM_PROMPT.to_string(),
            messages: vec![ClaudeMessage {
                role: "user".to_string(),
                content: format!(
                    "Generate a skill for the tool '{}' that: {}",
                    tool_name, requirement
                ),
            }],
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
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
                401 => "Invalid API key — check your ANTHROPIC_API_KEY".to_string(),
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
                            if let Ok(event) =
                                serde_json::from_str::<ClaudeEvent>(data)
                            {
                                match event.event_type.as_str() {
                                    "content_block_delta" => {
                                        if let Some(delta) = event.delta {
                                            if delta.delta_type.as_deref()
                                                == Some("text_delta")
                                            {
                                                if let Some(text) = delta.text {
                                                    if tx
                                                        .send(StreamToken::Token(text))
                                                        .await
                                                        .is_err()
                                                    {
                                                        return Ok(());
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    "message_stop" => {
                                        let _ = tx.send(StreamToken::Done).await;
                                        return Ok(());
                                    }
                                    "error" => {
                                        if let Some(err) = event.error {
                                            let _ = tx
                                                .send(StreamToken::Error(err.message))
                                                .await;
                                        }
                                        return Ok(());
                                    }
                                    _ => {}
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
        "Claude (Anthropic)"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
