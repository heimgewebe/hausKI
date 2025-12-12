use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    response::IntoResponse,
    routing::{any, post},
    Json, Router,
};
use std::time::Instant;

use crate::{AppState, NotImplementedResponse};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sync", post(sync_handler))
        .route("/fallback", post(fallback_handler))
        .route("/{*path}", any(not_implemented_handler))
        .route("/", any(not_implemented_handler))
}

// Roadmap P2: /cloud/fallback Endpoint with Policy-based Routing
// See docs/ist-stand-vs-roadmap.md
async fn fallback_handler(State(state): State<AppState>, req: Request<Body>) -> impl IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();
    tracing::warn!(%method, %uri, "access to unimplemented feature: cloud fallback");

    // In a real implementation, this would:
    // 1. Check RoutingPolicy
    // 2. Validate target URL with EgressGuard
    // 3. Forward request to upstream

    state.record_http_observation(
        method,
        "/cloud/fallback",
        StatusCode::NOT_IMPLEMENTED,
        Instant::now(),
    );

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(NotImplementedResponse {
            status: "not_implemented",
            hint: "Cloud fallback is planned (P2) - see docs/ist-stand-vs-roadmap.md",
            feature_id: "cloud_fallback",
        }),
    )
}

// Roadmap P2: /cloud/sync for synchronization
async fn sync_handler(State(state): State<AppState>, req: Request<Body>) -> impl IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();
    tracing::warn!(%method, %uri, "access to unimplemented feature: cloud sync");

    state.record_http_observation(
        method,
        "/cloud/sync",
        StatusCode::NOT_IMPLEMENTED,
        Instant::now(),
    );

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(NotImplementedResponse {
            status: "not_implemented",
            hint: "Cloud synchronization is planned - see docs/ist-stand-vs-roadmap.md",
            feature_id: "cloud_sync",
        }),
    )
}

async fn not_implemented_handler(
    State(state): State<AppState>,
    req: Request<Body>,
) -> impl IntoResponse {
    let method = req.method().clone();
    let uri = req.uri().clone();
    tracing::warn!(%method, %uri, "access to unimplemented feature: cloud (generic)");

    state.record_http_observation(
        method,
        "/cloud",
        StatusCode::NOT_IMPLEMENTED,
        Instant::now(),
    );

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(NotImplementedResponse {
            status: "not_implemented",
            hint: "Feature not implemented yet – see docs/inconsistencies.md#cloud",
            feature_id: "cloud",
        }),
    )
}
