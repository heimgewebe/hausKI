use std::time::Instant;

use axum::{
    extract::{Query, State},
    http::{Method, StatusCode},
    Json,
};
use hauski_indexd::SearchRequest;
use serde::{Deserialize, Serialize};

use utoipa::{IntoParams, ToSchema};

use crate::AppState;
// Used by utoipa's #[schema(example = json!(...))] attribute macros
#[allow(unused_imports)]
use serde_json::json;

/// Maximum number of matches returned by the `/ask` endpoint.
const MAX_K: usize = 100;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[schema(
    title = "AskHit",
    example = json!({
        "doc_id": "doc-42",
        "namespace": "default",
        "score": 0.87,
        "snippet": "HausKI keeps your knowledge organized.",
        "meta": {"source": "docs/intro.md"}
    })
)]
pub struct AskHit {
    pub doc_id: String,
    pub namespace: String,
    pub score: f32,
    pub snippet: String,
    pub meta: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[schema(
    title = "AskResponse",
    example = json!({
        "query": "What is HausKI?",
        "k": 5,
        "namespace": "default",
        "hits": [
            {
                "doc_id": "doc-42",
                "namespace": "default",
                "score": 0.87,
                "snippet": "HausKI keeps your knowledge organized.",
                "meta": {"source": "docs/intro.md"}
            }
        ]
    })
)]
pub struct AskResponse {
    pub query: String,
    pub k: usize,
    pub namespace: String,
    pub hits: Vec<AskHit>,
}

#[derive(Deserialize, Clone, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct AskParams {
    /// The query string for semantic search.
    pub q: String,
    /// Number of matches to return (server clamps the value between 1 and [`MAX_K`]).
    #[serde(default = "default_k")]
    #[param(default = 5, minimum = 1, maximum = 100)]
    #[schema(default = 5, minimum = 1, maximum = 100)]
    pub k: usize,
    /// Namespace to query within the index.
    #[serde(default = "default_ns")]
    #[param(default = "default")]
    #[schema(default = "default")]
    pub ns: String,
}

fn default_k() -> usize {
    5
}

fn default_ns() -> String {
    "default".to_string()
}

#[utoipa::path(
    get,
    path = "/ask",
    params(AskParams),
    responses(
        (status = 200, description = "Top-k semantic matches", body = AskResponse)
    ),
    tag = "core"
)]
pub async fn ask_handler(
    State(state): State<AppState>,
    Query(params): Query<AskParams>,
) -> Json<AskResponse> {
    let AskParams { q, k, ns } = params;
    let started = Instant::now();

    let limit = k.clamp(1, MAX_K);

    let request = SearchRequest {
        query: q.clone(),
        k: Some(limit),
        namespace: Some(ns.clone()),
        exclude_flags: None,
        min_trust_level: None,
        exclude_origins: None,
        context_profile: None,
        include_weights: false,
        emit_decision_snapshot: false,
    };

    let matches = state.index().search(&request).await;
    let hits = matches
        .into_iter()
        .map(|m| AskHit {
            doc_id: m.doc_id,
            namespace: m.namespace,
            score: m.score,
            snippet: m.text,
            meta: m.meta,
        })
        .collect();

    state.record_http_observation(Method::GET, "/ask", StatusCode::OK, started);

    Json(AskResponse {
        query: q,
        k: limit,
        namespace: ns,
        hits,
    })
}
