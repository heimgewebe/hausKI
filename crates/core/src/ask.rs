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

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct AskHit {
    pub doc_id: String,
    pub namespace: String,
    pub score: f32,
    pub snippet: String,
    pub meta: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, ToSchema)]
pub struct AskResponse {
    pub query: String,
    pub k: usize,
    pub namespace: String,
    pub hits: Vec<AskHit>,
}

#[derive(Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct AskParams {
    /// The query string for semantic search.
    pub q: String,
    /// Number of matches to return (server caps the value at 100).
    #[serde(default = "default_k")]
    #[param(default = 5)]
    #[schema(default = 5)]
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

    let request = SearchRequest {
        query: q.clone(),
        k: Some(k),
        namespace: Some(ns.clone()),
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
        k,
        namespace: ns,
        hits,
    })
}
