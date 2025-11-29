use axum::{
    extract::{FromRef, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{borrow::Cow, cmp::Ordering, collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::RwLock;

const DEFAULT_NAMESPACE: &str = "default";

pub type MetricsRecorder = dyn Fn(Method, &'static str, StatusCode, Instant) + Send + Sync;

fn normalize_namespace(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        DEFAULT_NAMESPACE.to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_namespace<'a>(namespace: Option<&'a str>) -> Cow<'a, str> {
    match namespace {
        Some(raw) => Cow::Owned(normalize_namespace(raw)),
        None => Cow::Borrowed(DEFAULT_NAMESPACE),
    }
}

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
        let UpsertRequest {
            doc_id,
            namespace,
            chunks,
            meta,
        } = payload;
        let namespace = normalize_namespace(&namespace);
        let mut store = self.inner.store.write().await;
        let namespace_store = store.entry(namespace.clone()).or_insert_with(HashMap::new);
        let ingested = chunks.len();
        namespace_store.insert(
            doc_id.clone(),
            DocumentRecord {
                doc_id,
                namespace: namespace.clone(),
                chunks,
                meta,
            },
        );
        ingested
    }

    pub async fn search(&self, request: &SearchRequest) -> Vec<SearchMatch> {
        let query = request.query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let store = self.inner.store.read().await;
        let namespace = resolve_namespace(request.namespace.as_deref());
        let Some(namespace_store) = store.get(namespace.as_ref()) else {
            return Vec::new();
        };
        let limit = request.k.unwrap_or(20).min(100);
        let query_lower = query.to_lowercase();
        let query_char_len = query_lower.chars().count();
        let query_byte_len = query_lower.len();

        let mut matches: Vec<SearchMatch> = Vec::new();
        for doc in namespace_store.values() {
            for (idx, chunk) in doc.chunks.iter().enumerate() {
                let Some(text) = chunk.text.as_ref() else {
                    continue;
                };

                let Some(score) =
                    substring_match_score(text, &query_lower, query_byte_len, query_char_len)
                else {
                    continue;
                };

                matches.push(SearchMatch {
                    doc_id: doc.doc_id.clone(),
                    namespace: doc.namespace.clone(),
                    chunk_id: chunk
                        .chunk_id
                        .clone()
                        .unwrap_or_else(|| format!("{}#{idx}", doc.doc_id)),
                    score,
                    text: text.clone(),
                    meta: if !chunk.meta.is_null() {
                        chunk.meta.clone()
                    } else {
                        doc.meta.clone()
                    },
                });
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        if matches.len() > limit {
            matches.truncate(limit);
        }
        matches
    }
}

fn substring_match_score(
    text: &str,
    query_lower: &str,
    query_byte_len: usize,
    query_char_len: usize,
) -> Option<f32> {
    if query_byte_len == 0 || query_char_len == 0 {
        return None;
    }

    let text_lower = text.to_lowercase();
    let mut count = 0;
    let mut remaining = text_lower.as_str();

    while let Some(pos) = remaining.find(query_lower) {
        count += 1;
        let advance = pos + query_byte_len;
        if advance >= remaining.len() {
            remaining = "";
        } else {
            remaining = &remaining[advance..];
        }
    }

    if count == 0 {
        return None;
    }

    let text_char_len = text_lower.chars().count().max(1);
    let matched_chars = count * query_char_len;
    Some((matched_chars as f32 / text_char_len as f32).min(1.0))
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
    use serde_json::json;
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

    #[tokio::test]
    async fn search_filters_results_by_query() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-rust".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-rust#0".into()),
                    text: Some("Rust programming language".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "rust"}),
            })
            .await;

        state
            .upsert(UpsertRequest {
                doc_id: "doc-cooking".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-cooking#0".into()),
                    text: Some("A collection of delicious recipes".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "cooking"}),
            })
            .await;

        let results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("default".into()),
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "doc-rust");
        assert!(results[0].text.to_lowercase().contains("rust"));
    }

    #[tokio::test]
    async fn trims_namespace_whitespace_on_upsert_and_search() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-trim".into(),
                namespace: "  custom  ".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-trim#0".into()),
                    text: Some("Rust namespaces".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "trim"}),
            })
            .await;

        let results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("custom".into()),
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].namespace, "custom");

        let spaced_results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("   custom   ".into()),
            })
            .await;

        assert_eq!(spaced_results.len(), 1);
        assert_eq!(spaced_results[0].doc_id, "doc-trim");
    }

    #[tokio::test]
    async fn empty_namespace_defaults_to_default_namespace() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-empty".into(),
                namespace: String::new(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-empty#0".into()),
                    text: Some("Hello default namespace".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "empty"}),
            })
            .await;

        let results = state
            .search(&SearchRequest {
                query: "hello".into(),
                k: Some(5),
                namespace: None,
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].namespace, DEFAULT_NAMESPACE);

        let spaced_results = state
            .search(&SearchRequest {
                query: "hello".into(),
                k: Some(5),
                namespace: Some("   ".into()),
            })
            .await;

        assert_eq!(spaced_results.len(), 1);
        assert_eq!(spaced_results[0].doc_id, "doc-empty");
        assert_eq!(spaced_results[0].namespace, DEFAULT_NAMESPACE);
    }
}
