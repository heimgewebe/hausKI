use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::AppState;
use hauski_memory as mem;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(title = "MemoryGetRequest", example = json!({"key":"greeting"}))]
pub struct MemoryGetRequest {
    pub key: String,
}
#[derive(Debug, Serialize, ToSchema)]
#[schema(title = "MemoryGetResponse", example = json!({"key":"greeting","value":"hi","ttl_sec":300,"pinned":false}))]
pub struct MemoryGetResponse {
    pub key: String,
    pub value: Option<String>,
    pub ttl_sec: Option<i64>,
    pub pinned: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(title = "MemorySetRequest", example = json!({"key":"greeting","value":"hi","ttl_sec":300,"pinned":false}))]
pub struct MemorySetRequest {
    pub key: String,
    pub value: String,
    #[serde(default)]
    pub ttl_sec: Option<i64>,
    #[serde(default)]
    pub pinned: Option<bool>,
}
#[derive(Debug, Serialize, ToSchema)]
#[schema(title = "MemorySetResponse", example = json!({"ok":true}))]
pub struct MemorySetResponse {
    pub ok: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(title = "MemoryEvictRequest", example = json!({"key":"greeting"}))]
pub struct MemoryEvictRequest {
    pub key: String,
}
#[derive(Debug, Serialize, ToSchema)]
#[schema(title = "MemoryEvictResponse", example = json!({"ok":true}))]
pub struct MemoryEvictResponse {
    pub ok: bool,
}

#[utoipa::path(
    post,
    path = "/memory/get",
    tag = "core",
    request_body = MemoryGetRequest,
    responses((status=200, body=MemoryGetResponse), (status=500, description="internal error"))
)]
pub async fn memory_get_handler(
    _state: State<AppState>,
    Json(req): Json<MemoryGetRequest>,
) -> Response {
    let store = mem::global();
    match store.get(&req.key) {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(MemoryGetResponse {
                key: req.key,
                value: Some(String::from_utf8_lossy(&item.value).into_owned()),
                ttl_sec: item.ttl_sec,
                pinned: Some(item.pinned),
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            Json(MemoryGetResponse {
                key: req.key,
                value: None,
                ttl_sec: None,
                pinned: None,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "failed to get memory item");
            (StatusCode::INTERNAL_SERVER_ERROR).into_response()
        }
    }
}

#[utoipa::path(
    post,
    path = "/memory/set",
    tag = "core",
    request_body = MemorySetRequest,
    responses((status=200, body=MemorySetResponse), (status=500, description="internal error"))
)]
pub async fn memory_set_handler(
    _state: State<AppState>,
    Json(req): Json<MemorySetRequest>,
) -> Response {
    let store = mem::global();
    match store.set(&req.key, req.value.as_bytes(), req.ttl_sec, req.pinned) {
        Ok(_) => (StatusCode::OK, Json(MemorySetResponse { ok: true })).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "failed to set memory item");
            (StatusCode::INTERNAL_SERVER_ERROR).into_response()
        }
    }
}

#[utoipa::path(
    post,
    path = "/memory/evict",
    tag = "core",
    request_body = MemoryEvictRequest,
    responses((status=200, body=MemoryEvictResponse), (status=500, description="internal error"))
)]
pub async fn memory_evict_handler(
    _state: State<AppState>,
    Json(req): Json<MemoryEvictRequest>,
) -> Response {
    let store = mem::global();
    match store.evict(&req.key) {
        Ok(ok) => (StatusCode::OK, Json(MemoryEvictResponse { ok })).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "failed to evict memory item");
            (StatusCode::INTERNAL_SERVER_ERROR).into_response()
        }
    }
}
