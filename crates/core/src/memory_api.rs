use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
// Used by utoipa's #[schema(example = json!(...))] attribute macros
#[allow(unused_imports)]
use serde_json::json;
use std::fs;
use std::path::Path;
use utoipa::ToSchema;

use crate::{record_memory_manual_eviction, AppState};
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

// ---------------------- Policy ----------------------
#[derive(Debug, Clone, Default, Deserialize)]
struct MemoryPolicy {
    #[serde(default)]
    default_ttl_sec: Option<i64>,
    #[serde(default)]
    pin_allowlist: Vec<String>,
}

static POLICY: OnceCell<MemoryPolicy> = OnceCell::new();

fn policy_load_once() -> &'static MemoryPolicy {
    POLICY.get_or_init(|| {
        // Reihenfolge:
        // 1) HAUSKI_MEMORY_POLICY_PATH
        // 2) ./policies/memory.yaml (repo-local)
        // 3) kein File -> Default
        let path = std::env::var("HAUSKI_MEMORY_POLICY_PATH")
            .ok()
            .unwrap_or_else(|| "policies/memory.yaml".to_string());
        let p = Path::new(&path);
        if p.exists() {
            match fs::read_to_string(p) {
                Ok(text) => match serde_yml::from_str::<MemoryPolicy>(&text) {
                    Ok(cfg) => cfg,
                    Err(err) => {
                        tracing::warn!("memory policy parse failed: {err} – using defaults");
                        MemoryPolicy::default()
                    }
                },
                Err(err) => {
                    tracing::warn!("memory policy read failed: {err} – using defaults");
                    MemoryPolicy::default()
                }
            }
        } else {
            MemoryPolicy::default()
        }
    })
}

fn is_pin_allowed(key: &str, allowlist: &[String]) -> bool {
    // sehr einfache Pattern-Logik: unterstützt "prefix:*"
    for pat in allowlist {
        if let Some(prefix) = pat.strip_suffix('*') {
            if key.starts_with(prefix) {
                return true;
            }
        } else if pat == key {
            return true;
        }
    }
    false
}

// ---------------------- Handlers ----------------------

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
    let key = req.key.clone();
    let result = mem::global().get(req.key).await;

    match result {
        Ok(Some(item)) => (
            StatusCode::OK,
            Json(MemoryGetResponse {
                key, // Use the cloned key here
                value: Some(String::from_utf8_lossy(&item.value).into_owned()),
                ttl_sec: item.ttl_sec,
                pinned: Some(item.pinned),
            }),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::OK,
            Json(MemoryGetResponse {
                key, // And here
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
    let pol = policy_load_once();

    // TTL: falls im Request nicht gesetzt, Policy-Default verwenden
    let ttl = req.ttl_sec.or(pol.default_ttl_sec);

    // pinned: falls im Request nicht gesetzt, Allowlist aus Policy prüfen
    // Note: This check is purely logical and doesn't block (much), so we can keep it here.
    let pinned = req.pinned.or_else(|| {
        if is_pin_allowed(&req.key, &pol.pin_allowlist) {
            Some(true)
        } else {
            None
        }
    });

    let result = mem::global()
        .set(req.key, req.value.into_bytes(), ttl, pinned)
        .await;

    match result {
        Ok(()) => (StatusCode::OK, Json(MemorySetResponse { ok: true })).into_response(),
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
    let result = mem::global().evict(req.key).await;

    match result {
        Ok(ok) => {
            if ok {
                // Nur inkrementieren, wenn wirklich ein Key gelöscht wurde.
                record_memory_manual_eviction();
            }
            (StatusCode::OK, Json(MemoryEvictResponse { ok })).into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, "failed to evict memory item");
            (StatusCode::INTERNAL_SERVER_ERROR).into_response()
        }
    }
}
