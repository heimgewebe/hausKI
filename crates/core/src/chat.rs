use std::{env, time::Instant};

use axum::{
    extract::State,
    http::{Method, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
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

const DEFAULT_MODEL: &str = "llama";

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct ChatMessage {
    /// Role of the message author (e.g. user, system, assistant).
    pub role: String,
    /// Natural language content submitted by the author.
    pub content: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChatResponse {
    /// Assistant message content produced by the upstream model.
    pub content: String,
    /// Model identifier reported back to clients (best effort).
    pub model: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatRequest {
    /// Sequence of messages forming the current conversation turn.
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ChatStubResponse {
    /// Static status marker highlighting that the endpoint is not wired yet.
    pub status: String,
    /// Human readable explanation for clients.
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
            status = 502,
            description = "Configured chat upstream returned an error",
            body = ChatStubResponse
        ),
        (
            status = 501,
            description = "Chat endpoint not yet implemented",
            body = ChatStubResponse
        )
    ),
    tag = "core"
)]
pub async fn post_chat(
    State(state): State<AppState>,
    Json(chat_request): Json<ChatRequest>,
) -> axum::response::Response {
    let chat_cfg = state.chat_cfg();

    if let Some(base_url) = chat_cfg.upstream_url.clone() {
        let started = Instant::now();
        let client = chat_cfg.client.clone();
        let model = chat_cfg
            .model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

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

    warn!("chat request received but no chat upstream is configured");
    let started = Instant::now();
    let status = StatusCode::NOT_IMPLEMENTED;
    state.record_http_observation(Method::POST, "/v1/chat", status, started);
    let payload = ChatStubResponse {
        status: "not_implemented".to_string(),
        message: "chat pipeline not wired yet, please configure HAUSKI_CHAT_UPSTREAM_URL"
            .to_string(),
    };
    (status, Json(payload)).into_response()
}
