use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::chat::ChatMessage;

#[derive(Debug, Serialize)]
struct OllamaChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaMessage>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
}

/// Call an Ollama-compatible `/api/chat` endpoint and return the first message.
pub async fn call_ollama_chat(
    client: &Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String> {
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let request = OllamaChatRequest {
        model,
        messages,
        stream: Some(false),
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;

    if !response.status().is_success() {
        return Err(anyhow!("upstream status {}", response.status()));
    }

    let parsed: OllamaChatResponse = response
        .json()
        .await
        .context("parse upstream json response")?;
    let reply = parsed
        .message
        .map(|m| m.content)
        .filter(|content| !content.is_empty())
        .unwrap_or_else(|| "(leer)".to_string());

    Ok(reply)
}
