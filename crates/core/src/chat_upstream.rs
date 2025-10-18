use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::chat::ChatMessage;

#[derive(Debug, Serialize)]
struct OpenAIChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatMessage],
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: String,
}

/// Call an OpenAI-compatible `/v1/chat/completions` endpoint and return the first choice.
pub async fn call_openai_compatible(
    client: &Client,
    base_url: &str,
    model: &str,
    messages: &[ChatMessage],
) -> Result<String> {
    let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));
    let request = OpenAIChatRequest {
        model,
        messages,
        temperature: None,
        max_tokens: None,
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .with_context(|| format!("POST {}", url))?;

    if !response.status().is_success() {
        return Err(anyhow!("upstream status {}", response.status()));
    }

    let parsed: OpenAIChatResponse = response
        .json()
        .await
        .context("parse upstream json response")?;
    let first = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("upstream returned no choices"))?;

    Ok(first.message.content)
}
