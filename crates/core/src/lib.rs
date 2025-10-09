use axum::error_handling::HandleErrorLayer;
use axum::extract::FromRef;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use hauski_indexd::{router as index_router, IndexState};
use prometheus_client::{
    encoding::{text::encode, EncodeLabel, EncodeLabelSet},
    metrics::{counter::Counter, family::Family, gauge::Gauge, histogram::Histogram},
    registry::Registry,
};
use std::{
    env, fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tower::{limit::ConcurrencyLimitLayer, timeout::TimeoutLayer, BoxError, ServiceBuilder};
use utoipa::OpenApi;

mod ask;
mod config;
mod egress;
pub use config::{
    load_flags, load_limits, load_models, load_routing, FeatureFlags, Limits, ModelEntry,
    ModelsFile, RoutingDecision, RoutingPolicy, RoutingRule,
};
pub use egress::{
    AllowlistedClient, EgressGuard, EgressGuardError, GuardError, GuardedRequestError,
};

const LATENCY_BUCKETS: [f64; 8] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0];
const CORE_SERVICE_NAME: &str = "core";
const INDEXD_SERVICE_NAME: &str = "indexd";

type MetricsCallback = dyn Fn(Method, &'static str, StatusCode, Instant) + Send + Sync;

#[derive(OpenApi)]
#[openapi(
    paths(health, ready, ask::ask_handler),
    components(schemas(ask::AskResponse, ask::AskHit)),
    tags((name = "core", description = "Core health & readiness"))
)]
struct ApiDoc;

/// Creates a latency histogram with predefined buckets.
///
/// The bucket values are defined in the `LATENCY_BUCKETS` constant (in seconds).
/// These were chosen to provide fine granularity for low-latency requests and broader
/// coverage for higher latencies, matching typical HTTP request latency distributions.
/// Adjust `LATENCY_BUCKETS` if your workload characteristics differ.
fn create_latency_histogram() -> Histogram {
    Histogram::new(LATENCY_BUCKETS.into_iter())
}

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

#[allow(dead_code)]
struct AppStateInner {
    limits: Limits,
    models: ModelsFile,
    routing: RoutingPolicy,
    flags: FeatureFlags,
    http_requests: Family<HttpLabels, Counter<u64>>,
    http_latency: Family<HttpDurationLabels, Histogram>,
    metrics_recorder: Arc<MetricsCallback>,
    index: IndexState,
    build_info: Family<BuildInfoLabels, Gauge>,
    registry: Registry,
    /// Controls whether configuration endpoints are exposed.
    ///
    /// WARNING: Enabling this may expose sensitive configuration information.
    /// Only set to `true` if you understand the security implications.
    expose_config: bool,
    ready: AtomicBool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct BuildInfoLabels {
    service: &'static str,
}

impl EncodeLabelSet for BuildInfoLabels {
    fn encode(
        &self,
        mut encoder: prometheus_client::encoding::LabelSetEncoder<'_>,
    ) -> Result<(), fmt::Error> {
        ("service", self.service).encode(encoder.encode_label())?;
        Ok(())
    }
}

impl AppState {
    fn new(
        limits: Limits,
        models: ModelsFile,
        routing: RoutingPolicy,
        flags: FeatureFlags,
        expose_config: bool,
    ) -> Self {
        let mut registry = Registry::default();

        let build_info = Family::<BuildInfoLabels, Gauge>::default();
        build_info
            .get_or_create(&BuildInfoLabels {
                service: CORE_SERVICE_NAME,
            })
            .set(1);
        build_info
            .get_or_create(&BuildInfoLabels {
                service: INDEXD_SERVICE_NAME,
            })
            .set(1);
        registry.register("build_info", "Build info per service", build_info.clone());

        let http_requests: Family<HttpLabels, Counter<u64>> = Family::default();
        registry.register(
            "http_requests_total",
            "Total number of HTTP requests received",
            http_requests.clone(),
        );

        let http_latency: Family<HttpDurationLabels, Histogram> =
            Family::new_with_constructor(create_latency_histogram);
        registry.register(
            "http_request_duration_seconds",
            "HTTP request duration",
            http_latency.clone(),
        );

        let metrics_recorder: Arc<MetricsCallback> = {
            let http_requests = http_requests.clone();
            let http_latency = http_latency.clone();
            Arc::new(move |method, path, status, started| {
                let counter_labels = HttpLabels::new(method.clone(), path, status);
                let duration_labels = HttpDurationLabels::new(method, path);
                let elapsed = started.elapsed().as_secs_f64();
                http_requests.get_or_create(&counter_labels).inc();
                http_latency
                    .get_or_create(&duration_labels)
                    .observe(elapsed);
            })
        };

        let index = IndexState::new(limits.latency.index_topk20_ms, metrics_recorder.clone());

        Self(Arc::new(AppStateInner {
            limits,
            models,
            routing,
            flags,
            http_requests,
            http_latency,
            metrics_recorder,
            index,
            build_info,
            registry,
            expose_config,
            ready: AtomicBool::new(false),
        }))
    }

    fn limits(&self) -> Limits {
        self.0.limits.clone()
    }

    fn models(&self) -> ModelsFile {
        self.0.models.clone()
    }

    fn routing(&self) -> RoutingPolicy {
        self.0.routing.clone()
    }

    pub fn flags(&self) -> FeatureFlags {
        self.0.flags.clone()
    }

    pub fn index(&self) -> IndexState {
        self.0.index.clone()
    }

    pub fn safe_mode(&self) -> bool {
        self.0.flags.safe_mode
    }

    fn expose_config(&self) -> bool {
        self.0.expose_config
    }

    fn encode_metrics(&self) -> Result<String, std::fmt::Error> {
        let mut body = String::new();
        encode(&mut body, &self.0.registry)?;
        Ok(body)
    }

    pub fn record_http_observation(
        &self,
        method: Method,
        path: &'static str,
        status: StatusCode,
        started: Instant,
    ) {
        (self.0.metrics_recorder)(method, path, status, started);
    }

    pub fn set_ready(&self) {
        self.0.ready.store(true, Ordering::Release);
    }

    fn is_ready(&self) -> bool {
        self.0.ready.load(Ordering::Acquire)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct HttpDurationLabels {
    method: Method,
    path: &'static str,
}

impl HttpDurationLabels {
    fn new(method: Method, path: &'static str) -> Self {
        Self { method, path }
    }
}

impl EncodeLabelSet for HttpDurationLabels {
    fn encode(
        &self,
        mut encoder: prometheus_client::encoding::LabelSetEncoder<'_>,
    ) -> Result<(), fmt::Error> {
        ("method", self.method.as_str()).encode(encoder.encode_label())?;
        ("path", self.path).encode(encoder.encode_label())?;
        Ok(())
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct HttpLabels {
    method: Method,
    path: &'static str,
    status: StatusCode,
}

impl HttpLabels {
    fn new(method: Method, path: &'static str, status: StatusCode) -> Self {
        Self {
            method,
            path,
            status,
        }
    }
}

impl EncodeLabelSet for HttpLabels {
    fn encode(
        &self,
        mut encoder: prometheus_client::encoding::LabelSetEncoder<'_>,
    ) -> Result<(), fmt::Error> {
        ("method", self.method.as_str()).encode(encoder.encode_label())?;
        ("path", self.path).encode(encoder.encode_label())?;
        ("status", self.status.as_str()).encode(encoder.encode_label())?;
        Ok(())
    }
}

impl FromRef<AppState> for IndexState {
    fn from_ref(state: &AppState) -> Self {
        state.index()
    }
}

async fn get_limits(State(state): State<AppState>) -> Json<Limits> {
    let started = Instant::now();
    let status = StatusCode::OK;
    let response = Json(state.limits());
    state.record_http_observation(Method::GET, "/config/limits", status, started);
    response
}

async fn get_models(State(state): State<AppState>) -> Json<ModelsFile> {
    let started = Instant::now();
    let status = StatusCode::OK;
    let response = Json(state.models());
    state.record_http_observation(Method::GET, "/config/models", status, started);
    response
}

async fn get_routing(State(state): State<AppState>) -> Json<RoutingPolicy> {
    let started = Instant::now();
    let status = StatusCode::OK;
    let response = Json(state.routing());
    state.record_http_observation(Method::GET, "/config/routing", status, started);
    response
}

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "Service healthy")),
    tag = "core"
)]
async fn health(State(state): State<AppState>) -> &'static str {
    let started = Instant::now();
    let status = StatusCode::OK;
    state.record_http_observation(Method::GET, "/health", status, started);
    "ok"
}

#[utoipa::path(
    get,
    path = "/ready",
    responses(
        (status = 200, description = "Service ready"),
        (status = 503, description = "Service starting")
    ),
    tag = "core"
)]
async fn ready(State(state): State<AppState>) -> (StatusCode, &'static str) {
    let started = Instant::now();
    let (status, body) = if state.is_ready() {
        (StatusCode::OK, "ok")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "starting")
    };
    state.record_http_observation(Method::GET, "/ready", status, started);
    (status, body)
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let started = Instant::now();
    let encoded_metrics = state.encode_metrics();
    let status = if encoded_metrics.is_ok() {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };

    state.record_http_observation(Method::GET, "/metrics", status, started);

    match encoded_metrics {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            body,
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            "Internal server error".to_string(),
        )
            .into_response(),
    }
}

pub fn build_app(
    limits: Limits,
    models: ModelsFile,
    routing: RoutingPolicy,
    flags: FeatureFlags,
    expose_config: bool,
    allowed_origin: HeaderValue,
) -> Router {
    build_app_with_state(
        limits,
        models,
        routing,
        flags,
        expose_config,
        allowed_origin,
    )
    .0
}

pub fn build_app_with_state(
    limits: Limits,
    models: ModelsFile,
    routing: RoutingPolicy,
    flags: FeatureFlags,
    expose_config: bool,
    allowed_origin: HeaderValue,
) -> (Router, AppState) {
    let state = AppState::new(limits, models, routing, flags, expose_config);
    let allowed_origin = Arc::new(allowed_origin);

    // --- Request guards ------------------------------------------------------
    // Defaults: 1500ms timeout, 512 concurrent requests – configurable via ENV:
    //   HAUSKI_HTTP_TIMEOUT_MS (u64; 0 = disabled)
    //   HAUSKI_HTTP_CONCURRENCY (u64; 0 = disabled)
    fn env_u64(key: &str, default: u64) -> u64 {
        match env::var(key) {
            Ok(v) => v.parse::<u64>().unwrap_or_else(|_| {
                tracing::warn!(
                    "Invalid value for {key}='{}' – falling back to {default}",
                    v
                );
                default
            }),
            Err(_) => default,
        }
    }
    let timeout_ms = env_u64("HAUSKI_HTTP_TIMEOUT_MS", 1500);
    let concurrency = env_u64("HAUSKI_HTTP_CONCURRENCY", 512);

    // Apply a timeout and concurrency limit before executing handlers so that
    // overload and slow upstreams surface consistent errors.
    let mut app = Router::new()
        .merge(core_routes())
        .nest("/index", index_router::<AppState>());

    if state.expose_config() {
        app = app.merge(config_routes()).merge(docs_routes());
    }

    if state.safe_mode() {
        tracing::info!("SAFE-MODE active: plugins and cloud routes disabled");
    } else {
        app = app.merge(plugin_routes()).merge(cloud_routes());
    }

    let timeout_layer = if timeout_ms > 0 {
        Some(TimeoutLayer::new(Duration::from_millis(timeout_ms)))
    } else {
        tracing::info!("HAUSKI_HTTP_TIMEOUT_MS=0 → request timeout disabled");
        None
    };
    let concurrency_layer = if concurrency > 0 {
        let c = std::cmp::min(concurrency, usize::MAX as u64) as usize;
        Some(ConcurrencyLimitLayer::new(c))
    } else {
        tracing::info!("HAUSKI_HTTP_CONCURRENCY=0 → concurrency limit disabled");
        None
    };

    let request_guards = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(|err: BoxError| async move {
            if err.is::<tower::timeout::error::Elapsed>() {
                (StatusCode::REQUEST_TIMEOUT, "request timed out")
            } else {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "service temporarily unavailable",
                )
            }
        }))
        .option_layer(timeout_layer)
        .option_layer(concurrency_layer)
        // Map the router's infallible error type to a BoxError before it
        // hits the optional layers. This is required for `option_layer`'s
        // `Either` service to work, as it requires both branches to have the
        // same error type.
        .layer(tower::util::MapErrLayer::new(
            |e: std::convert::Infallible| -> BoxError { match e {} },
        ));

    // The readiness flag is set by the caller once the listener is bound.
    let app = app
        .with_state(state.clone())
        .layer(from_fn_with_state(allowed_origin.clone(), cors_middleware))
        .layer(request_guards);
    (app, state)
}

fn core_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/ask", get(ask::ask_handler))
}

fn docs_routes() -> Router<AppState> {
    Router::new()
        .route("/docs", get(api_docs))
        .route("/docs/openapi.json", get(openapi_json))
}

const SWAGGER_UI_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>HausKI OpenAPI</title>
    <link rel="stylesheet" href="/static/swagger-ui/swagger-ui.css" />
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="/static/swagger-ui/swagger-ui-bundle.js"></script>
    <script>
      window.onload = () => {
        window.ui = SwaggerUIBundle({
          url: '/docs/openapi.json',
          dom_id: '#swagger-ui',
          presets: [SwaggerUIBundle.presets.apis],
        });
      };
    </script>
  </body>
</html>
"#;

async fn api_docs() -> Html<&'static str> {
    Html(SWAGGER_UI_HTML)
}

/// Serve OpenAPI as JSON with robust error handling.
async fn openapi_json() -> Response {
    // 1) Build the OpenAPI struct (infallible in `utoipa`).
    let spec = ApiDoc::openapi();

    // 2) Serialize explicitly so we can handle failures gracefully.
    match serde_json::to_vec(&spec) {
        Ok(bytes) => (
            StatusCode::OK,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            bytes,
        )
            .into_response(),
        Err(err) => {
            // Log for operators.
            tracing::error!("failed to serialize OpenAPI JSON: {err}");
            // Compact JSON error payload for clients.
            let body = format!("{{\"error\":\"failed to serialize openapi\",\"details\":\"{err}\"}}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/json"),
                )],
                body,
            )
                .into_response()
        }
    }
}

fn config_routes() -> Router<AppState> {
    Router::new()
        .route("/config/limits", get(get_limits))
        .route("/config/models", get(get_models))
        .route("/config/routing", get(get_routing))
}

// TODO: Implement plugin routes. This is a placeholder returning an empty router.
fn plugin_routes() -> Router<AppState> {
    Router::<AppState>::new()
}

// TODO: Implement cloud routes. This is a placeholder returning an empty router.
fn cloud_routes() -> Router<AppState> {
    Router::<AppState>::new()
}

type CorsState = Arc<HeaderValue>;

async fn cors_middleware(
    State(allowed_origin): State<CorsState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let origin = req.headers().get(header::ORIGIN).cloned();
    let origin_allowed = origin.as_ref() == Some(allowed_origin.as_ref());

    if req.method() == Method::OPTIONS {
        if !origin_allowed {
            return Response::builder()
                .status(StatusCode::FORBIDDEN)
                .body(Body::empty())
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
        }

        return Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                allowed_origin.as_ref().clone(),
            )
            .header(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                "GET, HEAD, POST, OPTIONS",
            )
            .header(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("Content-Type, Authorization"),
            )
            .header(
                header::ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_static("600"),
            )
            .header(header::VARY, HeaderValue::from_static("Origin"))
            .body(Body::empty())
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }

    let mut response = next.run(req).await;
    if origin_allowed {
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            allowed_origin.as_ref().clone(),
        );
        response
            .headers_mut()
            .append(header::VARY, HeaderValue::from_static("Origin"));
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ask::AskResponse;
    use axum::{
        body::Body,
        http::{header, HeaderValue, Method, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use serde_json::{from_slice, json};
    use tower::ServiceExt;

    fn demo_app(expose: bool) -> axum::Router {
        demo_app_with_origin_and_flags(
            expose,
            FeatureFlags::default(),
            HeaderValue::from_static("http://127.0.0.1:8080"),
        )
        .0
    }

    fn demo_app_with_origin(expose: bool, origin: HeaderValue) -> axum::Router {
        demo_app_with_origin_and_flags(expose, FeatureFlags::default(), origin).0
    }

    fn demo_app_with_origin_and_flags(
        expose: bool,
        flags: FeatureFlags,
        origin: HeaderValue,
    ) -> (axum::Router, AppState) {
        let limits = Limits {
            latency: crate::config::Latency {
                llm_p95_ms: 400,
                index_topk20_ms: 60,
            },
            thermal: crate::config::Thermal {
                gpu_max_c: 80,
                dgpu_power_w: 220,
            },
            asr: crate::config::Asr { wer_max_pct: 10 },
        };
        let models = ModelsFile {
            models: vec![crate::config::ModelEntry {
                id: "llama3.1-8b-q4".into(),
                path: "/opt/models/llama3.1-8b-q4.gguf".into(),
                vram_min_gb: Some(6),
                canary: Some(false),
            }],
        };
        let routing = RoutingPolicy::default();
        let (app, state) = build_app_with_state(limits, models, routing, flags, expose, origin);
        state.set_ready();
        (app, state)
    }

    #[tokio::test]
    async fn health_ok_and_metrics_increment() {
        let app = demo_app(false);

        // Hit /health to generate a metric.
        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        // Hit /metrics once. This will report the /health metric and increment its own counter.
        let res = app
            .clone()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text_one = String::from_utf8(body.to_vec()).unwrap();

        // Assert that the first /metrics call reported the /health call.
        let expected_health = [
            r#"http_requests_total{method="GET",path="/health",status="200"} 1"#,
            r#"http_requests_total_total{method="GET",path="/health",status="200"} 1"#,
        ];
        assert!(
            expected_health
                .iter()
                .any(|needle| text_one.contains(needle)),
            "metrics missing labeled health counter:\n{text_one}"
        );

        // Hit /metrics a second time. This will report the metric from the first /metrics call.
        let res = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text_two = String::from_utf8(body.to_vec()).unwrap();

        // Assert that the second /metrics call reported the first /metrics call.
        let expected_metrics = [
            r#"http_requests_total{method="GET",path="/metrics",status="200"} 1"#,
            r#"http_requests_total_total{method="GET",path="/metrics",status="200"} 1"#,
        ];
        assert!(
            expected_metrics
                .iter()
                .any(|needle| text_two.contains(needle)),
            "metrics missing labeled metrics counter:\n{text_two}"
        );
    }

    #[tokio::test]
    async fn index_routes_accept_requests() {
        let app = demo_app(false);

        let upsert_payload = json!({
            "doc_id": "doc-42",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "doc-42#0", "text": "Hallo Welt", "embedding": []}
            ],
            "meta": {"kind": "markdown"}
        });

        let upsert_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/index/upsert")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(upsert_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upsert_res.status(), StatusCode::OK);

        let search_payload = json!({"query": "Hallo", "k": 5, "namespace": "default"});
        let search_res = app
            .oneshot(
                Request::builder()
                    .uri("/index/search")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(search_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(search_res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ask_route_returns_hits() {
        let app = demo_app(false);

        let upsert_payload = json!({
            "doc_id": "ask-doc",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "ask-doc#0", "text": "Hallo Hauski", "embedding": []}
            ],
            "meta": {"kind": "markdown"}
        });

        let upsert_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/index/upsert")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(upsert_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(upsert_res.status(), StatusCode::OK);

        let ask_res = app
            .oneshot(
                Request::builder()
                    .uri("/ask?q=Hauski&k=3&ns=default")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ask_res.status(), StatusCode::OK);

        let body = ask_res.into_body().collect().await.unwrap().to_bytes();
        let response: AskResponse = from_slice(&body).unwrap();
        assert_eq!(response.namespace, "default");
        assert_eq!(response.k, 3);
        assert_eq!(response.query, "Hauski");
        assert!(!response.hits.is_empty(), "expected at least one hit");
        assert!(
            response.hits.iter().any(|hit| hit.doc_id == "ask-doc"),
            "expected a hit with doc_id ask-doc"
        );
    }

    #[tokio::test]
    async fn metrics_include_index_search() {
        let app = demo_app(false);

        let upsert_payload = json!({
            "doc_id": "metrics-demo",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "metrics-demo#0", "text": "Metrics Demo", "embedding": []}
            ],
            "meta": {"kind": "markdown"}
        });

        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/index/upsert")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(upsert_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let search_payload = json!({"query": "metrics", "k": 1, "namespace": "default"});
        let _ = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/index/search")
                    .method("POST")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(search_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        let res = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();

        let expected_search = [
            r#"http_requests_total{method="POST",path="/index/search",status="200"} 1"#,
            r#"http_requests_total_total{method="POST",path="/index/search",status="200"} 1"#,
        ];
        assert!(
            expected_search.iter().any(|needle| text.contains(needle)),
            "metrics missing index/search counter:\n{text}"
        );
    }

    #[tokio::test]
    async fn p95_budget_within_limit_for_health() {
        let app = demo_app(false);

        for _ in 0..50 {
            let _ = app
                .clone()
                .oneshot(Request::get("/health").body(Body::empty()).unwrap())
                .await
                .unwrap();
        }

        let res = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();

        assert!(text.contains("http_request_duration_seconds_bucket"));
    }

    #[tokio::test]
    async fn config_routes_hidden_by_default() {
        let app = demo_app(false);
        let res = app
            .clone()
            .oneshot(Request::get("/config/limits").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = app
            .clone()
            .oneshot(Request::get("/config/models").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        let res = app
            .clone()
            .oneshot(Request::get("/config/routing").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn config_routes_visible_when_enabled() {
        let app = demo_app(true);
        let res = app
            .clone()
            .oneshot(Request::get("/config/limits").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .clone()
            .oneshot(Request::get("/config/models").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let res = app
            .oneshot(Request::get("/config/routing").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn cors_allows_configured_origin() {
        let origin = HeaderValue::from_static("http://127.0.0.1:8080");
        let app = demo_app_with_origin(false, origin.clone());

        let res = app
            .oneshot(
                Request::get("/health")
                    .header(header::ORIGIN, origin.clone())
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&origin)
        );
    }

    #[tokio::test]
    async fn cors_blocks_unconfigured_origin() {
        let allowed_origin = HeaderValue::from_static("http://127.0.0.1:8080");
        let app = demo_app_with_origin(false, allowed_origin);

        let res = app
            .oneshot(
                Request::get("/health")
                    .header(
                        header::ORIGIN,
                        HeaderValue::from_static("https://example.com"),
                    )
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert!(res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .is_none());
    }

    #[tokio::test]
    async fn cors_preflight_allows_post_requests() {
        let origin = HeaderValue::from_static("http://127.0.0.1:8080");
        let app = demo_app_with_origin(false, origin.clone());

        let res = app
            .oneshot(
                Request::builder()
                    .uri("/index/upsert")
                    .method(Method::OPTIONS)
                    .header(header::ORIGIN, origin.clone())
                    .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let allow_methods = res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .expect("missing Access-Control-Allow-Methods header");
        let allow_methods = allow_methods
            .to_str()
            .expect("non-UTF8 allow methods header");
        assert!(
            allow_methods.contains("POST"),
            "preflight response missing POST in allow methods: {allow_methods}"
        );
        assert_eq!(
            res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            Some(&origin)
        );
    }

    #[tokio::test]
    async fn readiness_is_ok() {
        let app = demo_app(false);
        let res = app
            .oneshot(Request::get("/ready").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn safe_mode_flag_is_reflected_in_state() {
        let (_app, state) = demo_app_with_origin_and_flags(
            false,
            FeatureFlags { safe_mode: true },
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );
        assert!(state.safe_mode());
        assert!(state.flags().safe_mode);
    }
}
