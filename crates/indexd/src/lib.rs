use axum::{
    extract::{FromRef, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::RwLock;

const DEFAULT_NAMESPACE: &str = "default";

pub type MetricsRecorder = dyn Fn(Method, &'static str, StatusCode, Instant) + Send + Sync;

#[derive(Clone)]
pub struct IndexState {
    inner: Arc<IndexInner>,
}

struct IndexInner {
    store: RwLock<HashMap<String, NamespaceStore>>,
    metrics: Arc<MetricsRecorder>,
    budget_ms: u64,
}

type NamespaceStore = HashMap<String, DocumentRecord>;

#[derive(Clone, Debug)]
struct DocumentRecord {
    doc_id: String,
    namespace: String,
    chunks: Vec<ChunkPayload>,
    meta: Value,
}

impl IndexState {
    pub fn new(budget_ms: u64, metrics: Arc<MetricsRecorder>) -> Self {
        Self {
            inner: Arc::new(IndexInner {
                store: RwLock::new(HashMap::new()),
                metrics,
                budget_ms,
            }),
        }
    }

    pub fn budget_ms(&self) -> u64 {
        self.inner.budget_ms
    }

    fn record(&self, method: Method, path: &'static str, status: StatusCode, started: Instant) {
        (self.inner.metrics)(method, path, status, started);
    }

    async fn upsert(&self, payload: UpsertRequest) -> usize {
        let mut store = self.inner.store.write().await;
        let namespace_store = store
            .entry(payload.namespace.clone())
            .or_insert_with(HashMap::new);
        let ingested = payload.chunks.len();
        namespace_store.insert(
            payload.doc_id.clone(),
            DocumentRecord {
                doc_id: payload.doc_id,
                namespace: payload.namespace,
                chunks: payload.chunks,
                meta: payload.meta,
            },
        );
        ingested
    }

    pub async fn search(&self, request: &SearchRequest) -> Vec<SearchMatch> {
        let store = self.inner.store.read().await;
        let namespace = request.namespace.as_deref().unwrap_or(DEFAULT_NAMESPACE);
        let Some(namespace_store) = store.get(namespace) else {
            return Vec::new();
        };
        let limit = request.k.unwrap_or(20).min(100);
        namespace_store
            .values()
            .flat_map(|doc| {
                doc.chunks
                    .iter()
                    .enumerate()
                    .filter_map(move |(idx, chunk)| {
                        chunk.text.as_ref().map(|text| SearchMatch {
                            doc_id: doc.doc_id.clone(),
                            namespace: doc.namespace.clone(),
                            chunk_id: chunk
                                .chunk_id
                                .clone()
                                .unwrap_or_else(|| format!("{}#{idx}", doc.doc_id)),
                            score: 0.0,
                            text: text.clone(),
                            meta: doc.meta.clone(),
                        })
                    })
            })
            .take(limit)
            .collect()
    }
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    IndexState: FromRef<S>,
{
    Router::<S>::new()
        .route("/upsert", post(upsert_handler))
        .route("/search", post(search_handler))
}

async fn upsert_handler(
    State(state): State<IndexState>,
    Json(payload): Json<UpsertRequest>,
) -> Response {
    let started = Instant::now();
    if payload.namespace.trim().is_empty() {
        state.record(
            Method::POST,
            "/index/upsert",
            StatusCode::BAD_REQUEST,
            started,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "namespace must not be empty" })),
        )
            .into_response();
    }
    let ingested = state.upsert(payload).await;
    state.record(Method::POST, "/index/upsert", StatusCode::OK, started);
    (
        StatusCode::OK,
        Json(UpsertResponse {
            status: "queued".into(),
            ingested,
        }),
    )
        .into_response()
}

async fn search_handler(
    State(state): State<IndexState>,
    Json(payload): Json<SearchRequest>,
) -> Response {
    let started = Instant::now();
    if payload
        .namespace
        .as_deref()
        .is_some_and(|namespace| namespace.trim().is_empty())
    {
        state.record(
            Method::POST,
            "/index/search",
            StatusCode::BAD_REQUEST,
            started,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "namespace must not be empty" })),
        )
            .into_response();
    }
    let matches = state.search(&payload).await;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    state.record(Method::POST, "/index/search", StatusCode::OK, started);
    (
        StatusCode::OK,
        Json(SearchResponse {
            matches,
            latency_ms,
            budget_ms: state.budget_ms(),
        }),
    )
        .into_response()
}

#[derive(Debug, Deserialize)]
pub struct UpsertRequest {
    pub doc_id: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub chunks: Vec<ChunkPayload>,
    #[serde(default)]
    pub meta: Value,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChunkPayload {
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub meta: Value,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub k: Option<usize>,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpsertResponse {
    pub status: String,
    pub ingested: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub matches: Vec<SearchMatch>,
    pub latency_ms: f64,
    pub budget_ms: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchMatch {
    pub doc_id: String,
    pub namespace: String,
    pub chunk_id: String,
    pub score: f32,
    pub text: String,
    pub meta: Value,
}

fn default_namespace() -> String {
    DEFAULT_NAMESPACE.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use tower::ServiceExt;

    #[tokio::test]
    async fn upsert_and_search_return_ok() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));
        let app = router().with_state(state);

        let payload = serde_json::json!({
            "doc_id": "doc-1",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "doc-1#0", "text": "Hallo Welt", "embedding": []}
            ],
            "meta": {"kind": "markdown"}
        });

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/upsert")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let search_payload = serde_json::json!({"query": "Hallo", "k": 1, "namespace": "default"});
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/search")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(search_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
