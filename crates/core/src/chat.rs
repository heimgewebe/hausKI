use std::{env, time::Instant};

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use serde_json::json;
use tracing::{debug, warn};
use utoipa::ToSchema;

use crate::{chat_upstream::call_ollama_chat, AppState};

#[derive(Debug, Clone)]
pub struct ChatCfg {
    pub upstream_url: Option<String>,
    pub model: Option<String>,
    pub client: reqwest::Client,
}

impl ChatCfg {
    pub fn new(upstream_url: Option<String>, model: Option<String>) -> Self {
        Self {
            upstream_url,
            model,
            client: reqwest::Client::new(),
        }
    }

    pub fn from_env_and_flags(flag_upstream: Option<String>, flag_model: Option<String>) -> Self {
        let upstream_env =
            env_var("HAUSKI_CHAT_UPSTREAM_URL").or_else(|| env_var("CHAT_UPSTREAM_URL"));
        let upstream_url = upstream_env.or(flag_upstream);
        let model = env_var("HAUSKI_CHAT_MODEL").or(flag_model);

        Self::new(upstream_url, model)
    }
}

fn env_var(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Err(_) => None,
    }
}

/// Allowed roles for chat messages.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(title = "ChatRole", example = "user")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    #[serde(alias = "tool", alias = "function")]
    Tool,
}

const MAX_MESSAGES: usize = 32;
const MAX_CHARS_PER_MSG: usize = 16_000;
const RETRY_AFTER_SECS: &str = "30";

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(title = "ChatMessage", example = json!({"role":"user","content":"Hallo HausKI?"}))]
pub struct ChatMessage {
    /// Role of the message author (e.g. user, system, assistant).
    pub role: ChatRole,
    /// Natural language content submitted by the author.
    pub content: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(title = "ChatResponse", example = json!({"content":"Hallo! Wie kann ich helfen?","model":"llama3.1-8b-q4"}))]
pub struct ChatResponse {
    /// Assistant message content produced by the upstream model.
    pub content: String,
    /// Model identifier reported back to clients (best effort).
    pub model: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(title = "ChatRequest", example = json!({"messages":[{"role":"user","content":"Hallo HausKI?"}]}))]
pub struct ChatRequest {
    /// Sequence of messages forming the current conversation turn.
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(title = "ChatStubResponse", example = json!({
    "status": "not_implemented",
    "message": "chat pipeline not wired yet, please configure HAUSKI_CHAT_UPSTREAM_URL"
}))]
pub struct ChatStubResponse {
    /// Stub information for unimplemented or failed chat routes.
    pub status: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_env_vars() {
        for key in [
            "HAUSKI_CHAT_UPSTREAM_URL",
            "CHAT_UPSTREAM_URL",
            "HAUSKI_CHAT_MODEL",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    #[serial]
    fn from_env_prefers_primary_env_over_flag() {
        clear_env_vars();
        std::env::set_var("HAUSKI_CHAT_UPSTREAM_URL", " https://example.invalid/chat ");
        std::env::set_var("HAUSKI_CHAT_MODEL", " llama-3.1 ");

        let cfg = ChatCfg::from_env_and_flags(
            Some("https://flag.invalid".to_string()),
            Some("flag-model".to_string()),
        );

        assert_eq!(
            cfg.upstream_url.as_deref(),
            Some("https://example.invalid/chat")
        );
        assert_eq!(cfg.model.as_deref(), Some("llama-3.1"));

        clear_env_vars();
    }

    #[test]
    #[serial]
    fn from_env_uses_legacy_variable_when_new_is_missing() {
        clear_env_vars();
        std::env::set_var("CHAT_UPSTREAM_URL", "http://legacy.invalid");

        let cfg = ChatCfg::from_env_and_flags(None, Some("flag-model".to_string()));

        assert_eq!(cfg.upstream_url.as_deref(), Some("http://legacy.invalid"));
        assert_eq!(cfg.model.as_deref(), Some("flag-model"));

        clear_env_vars();
    }

    #[test]
    #[serial]
    fn from_env_falls_back_to_flag_when_env_empty() {
        clear_env_vars();
        std::env::set_var("HAUSKI_CHAT_UPSTREAM_URL", "   ");

        let cfg = ChatCfg::from_env_and_flags(
            Some("https://flag.invalid".to_string()),
            Some("flag-model".to_string()),
        );

        assert_eq!(cfg.upstream_url.as_deref(), Some("https://flag.invalid"));
        assert_eq!(cfg.model.as_deref(), Some("flag-model"));

        clear_env_vars();
    }

    #[test]
    #[serial]
    fn from_env_returns_none_when_flags_absent() {
        clear_env_vars();

        let cfg = ChatCfg::from_env_and_flags(None, None);

        assert!(cfg.upstream_url.is_none());
        assert!(cfg.model.is_none());

        clear_env_vars();
    }
}

/// Lightweight input validation to protect upstreams and keep error reporting clear.
fn validate_chat_request(req: &ChatRequest) -> Result<(), ChatStubResponse> {
    if req.messages.is_empty() {
        return Err(ChatStubResponse {
            status: "bad_request".to_string(),
            message: "messages must not be empty".to_string(),
        });
    }

    if req.messages.len() > MAX_MESSAGES {
        return Err(ChatStubResponse {
            status: "too_many_messages".to_string(),
            message: format!("messages limited to {MAX_MESSAGES}"),
        });
    }

    if let Some((index, _)) = req
        .messages
        .iter()
        .enumerate()
        .find(|(_, message)| message.content.trim().is_empty())
    {
        return Err(ChatStubResponse {
            status: "bad_request".to_string(),
            message: format!("message {index} must not be empty"),
        });
    }

    if let Some((index, _)) = req
        .messages
        .iter()
        .enumerate()
        .find(|(_, message)| message.content.chars().count() > MAX_CHARS_PER_MSG)
    {
        return Err(ChatStubResponse {
            status: "message_too_long".to_string(),
            message: format!("message {index} exceeds {MAX_CHARS_PER_MSG} chars"),
        });
    }

    Ok(())
}

// Hinweis: Wir dokumentieren die `Retry-After`-Header für 503-Antworten.
#[utoipa::path(
    post,
    path = "/v1/chat",
    request_body = ChatRequest,
    responses(
        (
            status = 200,
            description = "Successful chat response via configured upstream",
            body = ChatResponse
        ),
        (
            status = 400,
            description = "Invalid chat request payload",
            body = ChatStubResponse
        ),
        (
            status = 502,
            description = "Configured chat upstream returned an error",
            body = ChatStubResponse
        ),
        (
            status = 501,
            description = "Chat endpoint not implemented",
            body = ChatStubResponse
        ),
        (
            status = 503,
            description = "Chat endpoint not currently available",
            body = ChatStubResponse,
            headers(
                ("Retry-After" = String, description = "Client backoff in seconds")
            )
        )
    ),
    tag = "core"
)]
pub async fn chat_handler(
    State(state): State<AppState>,
    Json(chat_request): Json<ChatRequest>,
) -> axum::response::Response {
    let started = Instant::now();

    if let Err(payload) = validate_chat_request(&chat_request) {
        let status = StatusCode::BAD_REQUEST;
        state.record_http_observation(Method::POST, "/v1/chat", status, started);
        return (status, Json(payload)).into_response();
    }

    let chat_cfg = state.chat_cfg();
    if let Some(base_url) = chat_cfg.upstream_url.clone() {
        if let Some(model) = chat_cfg.model.clone() {
            let client = chat_cfg.client.clone();

            match call_ollama_chat(&client, &base_url, &model, &chat_request.messages).await {
                Ok(content) => {
                    let status = StatusCode::OK;
                    state.record_http_observation(Method::POST, "/v1/chat", status, started);
                    debug!(
                        base_url = %base_url,
                        status = %status,
                        model = %model,
                        "chat upstream succeeded"
                    );
                    return (status, Json(ChatResponse { content, model })).into_response();
                }
                Err(err) => {
                    let status = StatusCode::BAD_GATEWAY;
                    state.record_http_observation(Method::POST, "/v1/chat", status, started);
                    debug!(base_url = %base_url, error = %err, "chat upstream failed");
                    let payload = ChatStubResponse {
                        status: "upstream_error".to_string(),
                        message: format!("chat upstream failed: {err}"),
                    };
                    return (status, Json(payload)).into_response();
                }
            }
        }

        warn!("chat request received but no chat model is configured");
        let status = StatusCode::SERVICE_UNAVAILABLE;
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::RETRY_AFTER,
            HeaderValue::from_static(RETRY_AFTER_SECS),
        );
        state.record_http_observation(Method::POST, "/v1/chat", status, started);
        let payload = ChatStubResponse {
            status: "unavailable".to_string(),
            message: "missing HAUSKI_CHAT_MODEL".to_string(),
        };
        return (status, headers, Json(payload)).into_response();
    }

    warn!("chat request received but no chat upstream is configured");
    let status = StatusCode::SERVICE_UNAVAILABLE;
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::RETRY_AFTER,
        HeaderValue::from_static("30"),
    );
    state.record_http_observation(Method::POST, "/v1/chat", status, started);
    let payload = ChatStubResponse {
        status: "unavailable".to_string(),
        message: "chat pipeline not wired yet, please configure HAUSKI_CHAT_UPSTREAM_URL"
            .to_string(),
    };
    (status, headers, Json(payload)).into_response()
}
