use std::time::Instant;

use axum::{
    extract::State,
    http::{Method, StatusCode},
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::debug;
use utoipa::ToSchema;

use crate::AppState;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatMessage {
    /// Role of the message author (e.g. user, system, assistant).
    pub role: String,
    /// Natural language content submitted by the author.
    pub content: String,
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

#[utoipa::path(
    post,
    path = "/v1/chat",
    request_body = ChatRequest,
    responses(
        (
            status = 501,
            description = "Chat endpoint not yet implemented",
            body = ChatStubResponse
        )
    ),
    tag = "core"
)]
pub async fn chat_handler(
    State(state): State<AppState>,
    Json(chat_request): Json<ChatRequest>,
) -> (StatusCode, Json<ChatStubResponse>) {
    let mut has_user_message = false;
    let mut has_assistant_message = false;
    let mut non_empty_messages = 0usize;

    for message in &chat_request.messages {
        match message.role.as_str() {
            "user" => has_user_message = true,
            "assistant" => has_assistant_message = true,
            _ => {}
        }

        if !message.content.trim().is_empty() {
            non_empty_messages += 1;
        }
    }

    debug!(
        total_messages = chat_request.messages.len(),
        has_user_message, has_assistant_message, non_empty_messages, "received chat request (stub)"
    );
    let started = Instant::now();
    let status = StatusCode::NOT_IMPLEMENTED;
    state.record_http_observation(Method::POST, "/v1/chat", status, started);

    let payload = ChatStubResponse {
        status: "not_implemented".to_string(),
        message: "chat pipeline not wired yet".to_string(),
    };

    (status, Json(payload))
}
