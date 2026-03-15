// Google Gemini — streamGenerateContent (SSE)
// Endpoint: POST https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse&key={API_KEY}
// Docs: https://ai.google.dev/api/generate-content

use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

// ── Model listing ─────────────────────────────────────────────────────────────
// GET https://generativelanguage.googleapis.com/v1beta/models?key={API_KEY}
// Response: { "models": [{ "name": "models/gemini-2.0-flash", "supportedGenerationMethods": [...] }] }

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Option<Vec<GeminiModelEntry>>,
}

#[derive(Deserialize)]
struct GeminiModelEntry {
    name: String, // e.g. "models/gemini-2.0-flash"
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

/// Fetch available Gemini models that support content generation.
pub async fn fetch_models(api_key: &str) -> Vec<String> {
    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={}",
        api_key
    );
    let resp = client.get(&url).send().await;

    match resp {
        Ok(r) if r.status().is_success() => {
            r.json::<GeminiModelsResponse>()
                .await
                .map(|m| {
                    m.models
                        .unwrap_or_default()
                        .into_iter()
                        // Only keep models that support generateContent
                        .filter(|m| {
                            m.supported_generation_methods
                                .as_ref()
                                .map(|methods| methods.iter().any(|m| m == "generateContent"))
                                .unwrap_or(false)
                        })
                        // Strip "models/" prefix to get the bare model id
                        .map(|m| {
                            m.name
                                .strip_prefix("models/")
                                .unwrap_or(&m.name)
                                .to_string()
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        _ => vec![],
    }
}

use super::AIProvider;
use crate::types::StreamToken;

pub struct GeminiProvider {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }
}

// ── Request types ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct GeminiRequest {
    #[serde(rename = "systemInstruction")]
    system_instruction: GeminiMessageContent,
    contents: Vec<GeminiMessageContent>,
    tools: Vec<GeminiTool>,
}

/// Gemini grounding tool — Google executes the search server-side and
/// injects results directly into the model context. No client loop needed.
#[derive(Serialize)]
struct GeminiTool {
    #[serde(rename = "googleSearch")]
    google_search: GeminiGoogleSearch,
}

#[derive(Serialize)]
struct GeminiGoogleSearch {}

#[derive(Serialize)]
struct GeminiMessageContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

// ── Response types ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GeminiStreamChunk {
    candidates: Option<Vec<GeminiCandidate>>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiCandidateContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct GeminiCandidateContent {
    parts: Option<Vec<GeminiResponsePart>>,
}

#[derive(Deserialize)]
struct GeminiResponsePart {
    text: Option<String>,
}


// ── Provider implementation ────────────────────────────────────────────────────

#[async_trait]
impl AIProvider for GeminiProvider {
    async fn generate_skill(
        &self,
        tool_name: &str,
        requirement: &str,
        tx: mpsc::Sender<StreamToken>,
    ) -> Result<()> {
        // Gemini API key is passed as a query parameter, not a header
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.model, self.api_key
        );

        let body = GeminiRequest {
            system_instruction: GeminiMessageContent {
                role: None,
                parts: vec![GeminiPart {
                    text: super::SKILL_SYSTEM_PROMPT.to_string(),
                }],
            },
            contents: vec![GeminiMessageContent {
                role: Some("user".to_string()),
                parts: vec![GeminiPart {
                    text: super::skill_user_message(tool_name, requirement),
                }],
            }],
            tools: vec![GeminiTool {
                google_search: GeminiGoogleSearch {},
            }],
        };

        let response = self
            .client
            .post(&url)
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
                400 => "Bad request — verify your model name (e.g. gemini-2.0-flash)".to_string(),
                401 | 403 => "Invalid API key — check your GEMINI_API_KEY".to_string(),
                429 => "Rate limited — please wait before retrying".to_string(),
                s => format!("Gemini error (HTTP {})", s),
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

            // Process all complete lines in the buffer
            loop {
                match buf.find('\n') {
                    None => break,
                    Some(pos) => {
                        let line = buf[..pos].trim_end_matches('\r').to_string();
                        buf = buf[pos + 1..].to_string();

                        // SSE data line: "data: {...}"
                        if let Some(data) = line.strip_prefix("data: ") {
                            match serde_json::from_str::<GeminiStreamChunk>(data) {
                                Ok(chunk) => {
                                    if let Some(candidates) = chunk.candidates {
                                        if let Some(candidate) = candidates.into_iter().next() {
                                            // Extract text tokens from parts
                                            if let Some(content) = candidate.content {
                                                if let Some(parts) = content.parts {
                                                    for part in parts {
                                                        if let Some(text) = part.text {
                                                            if !text.is_empty()
                                                                && tx
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
                                            // finishReason present → generation complete
                                            if candidate.finish_reason.is_some() {
                                                let _ = tx.send(StreamToken::Done).await;
                                                return Ok(());
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    // Ignore malformed chunks (comments, keep-alives, etc.)
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
        "Google Gemini"
    }

    fn model(&self) -> &str {
        &self.model
    }
}
