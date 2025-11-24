use axum::error_handling::HandleErrorLayer;
use axum::extract::FromRef;
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
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
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tower::{limit::ConcurrencyLimitLayer, timeout::TimeoutLayer, BoxError, ServiceBuilder};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use once_cell::sync::OnceCell;
use prometheus_client::metrics::counter::Counter as PromCounter;
use prometheus_client::metrics::gauge::Gauge as PromGauge;
use hauski_memory as memory;

mod ask;
mod assist;
mod chat;
mod chat_upstream;
mod config;
mod egress;
mod memory_api;
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
    paths(
        health, healthz, ready,
        ask::ask_handler, chat::chat_handler,
        memory_api::memory_get_handler, memory_api::memory_set_handler, memory_api::memory_evict_handler,
        assist::assist_handler
    ),
    components(
        schemas(
            ask::AskResponse,
            ask::AskHit,
            chat::ChatRequest,
            chat::ChatMessage,
            chat::ChatStubResponse,
            chat::ChatResponse,
            memory_api::MemoryGetRequest, memory_api::MemoryGetResponse,
            memory_api::MemorySetRequest, memory_api::MemorySetResponse,
            memory_api::MemoryEvictRequest, memory_api::MemoryEvictResponse,
            assist::AssistRequest,
            assist::AssistResponse
        )
    ),
    tags(
        (name = "core", description = "Core service endpoints")
    )
)]
pub struct ApiDoc;

// ---- Memory metrics handles (set at startup) -------------------------------
static MEMORY_ITEMS_PINNED_GAUGE: OnceCell<PromGauge> = OnceCell::new();
static MEMORY_ITEMS_UNPINNED_GAUGE: OnceCell<PromGauge> = OnceCell::new();
static MEMORY_EVICTIONS_EXPIRED: OnceCell<PromCounter> = OnceCell::new();
static MEMORY_EVICTIONS_MANUAL: OnceCell<PromCounter> = OnceCell::new();

/// Inkrement aus dem /memory/evict-Handler (ohne AppState zu ändern).
pub(crate) fn record_memory_manual_eviction() {
    if let Some(c) = MEMORY_EVICTIONS_MANUAL.get() {
        c.inc();
    }
}

/// Creates a latency histogram with predefined buckets.
fn create_latency_histogram() -> Histogram {
    Histogram::new(LATENCY_BUCKETS)
}

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

#[allow(dead_code)]
struct AppStateInner {
    limits: Limits,
    models: ModelsFile,
    routing: RoutingPolicy,
    flags: FeatureFlags,
    chat_cfg: Arc<chat::ChatCfg>,
    http_requests: Family<HttpLabels, Counter<u64>>,
    http_latency: Family<HttpDurationLabels, Histogram>,
    metrics_recorder: Arc<MetricsCallback>,
    index: IndexState,
    build_info: Family<BuildInfoLabels, Gauge>,
    registry: Mutex<Registry>,
    /// HTTP-Client für ausgehende Anfragen (z. B. /assist, Plugins).
    http_client: reqwest::Client,
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
        encoder: &mut prometheus_client::encoding::LabelSetEncoder<'_>,
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
        chat_cfg: Arc<chat::ChatCfg>,
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
            "http_requests",
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

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|e| {
                tracing::warn!("failed to build http client, falling back to default: {}", e);
                reqwest::Client::new()
            });

        Self(Arc::new(AppStateInner {
            limits,
            models,
            routing,
            flags,
            chat_cfg,
            http_requests,
            http_latency,
            metrics_recorder,
            index,
            build_info,
            registry: Mutex::new(registry),
            http_client,
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

    pub fn chat_cfg(&self) -> Arc<chat::ChatCfg> {
        self.0.chat_cfg.clone()
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
        let registry = self.0.registry.lock().unwrap();
        encode(&mut body, &registry)?;
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

    pub fn http_client(&self) -> reqwest::Client {
        self.0.http_client.clone()
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
        encoder: &mut prometheus_client::encoding::LabelSetEncoder<'_>,
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
        encoder: &mut prometheus_client::encoding::LabelSetEncoder<'_>,
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

impl FromRef<AppState> for reqwest::Client {
    fn from_ref(state: &AppState) -> Self {
        state.http_client()
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
    path = "/healthz",
    responses((status = 200, description = "Service healthy")),
    tag = "core"
)]
async fn healthz(State(state): State<AppState>) -> &'static str {
    let started = Instant::now();
    let status = StatusCode::OK;
    state.record_http_observation(Method::GET, "/healthz", status, started);
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
    let chat_cfg = Arc::new(chat::ChatCfg::from_env_and_flags(
        flags.chat_upstream_url.clone(),
        flags.chat_model.clone(),
    ));
    let state = AppState::new(limits, models, routing, flags, chat_cfg, expose_config);
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

    // Initialize memory subsystem. This is fallible, so we capture the result.
    let memory_initialized = hauski_memory::init_default().map_err(|e| {
        tracing::error!(error = ?e, "failed to initialize memory subsystem");
        e
    }).is_ok();

    if state.expose_config() {
        // OpenAPI UI under /docs, spec under /api-docs/openapi.json
        let swagger = SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi());

        app = app.merge(config_routes()).merge(swagger);

        // Conditionally add memory routes if the subsystem is up.
        if memory_initialized {
            app = app.merge(memory_routes());
        }
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

    // ---- Memory metrics registration & poller -------------------------------
    if memory_initialized {
        use prometheus_client::registry::Unit;
        // Register metrics into the existing registry
        let pinned_g = PromGauge::default();
        let unpinned_g = PromGauge::default();
        let expired_c = PromCounter::default();
        let manual_c = PromCounter::default();

        let mut registry = state.0.registry.lock().unwrap();
        registry.register_with_unit(
            "memory_items_pinned",
            "Number of pinned items in hauski-memory",
            Unit::Other("Count".into()),
            pinned_g.clone(),
        );
        registry.register_with_unit(
            "memory_items_unpinned",
            "Number of unpinned items in hauski-memory",
            Unit::Other("Count".into()),
            unpinned_g.clone(),
        );
        registry.register_with_unit(
            "memory_evictions_expired_total",
            "Total number of TTL-based (expired) evictions performed by janitor",
            Unit::Other("Count".into()),
            expired_c.clone(),
        );
        registry.register_with_unit(
            "memory_evictions_manual_total",
            "Total number of manual evictions via API",
            Unit::Other("Count".into()),
            manual_c.clone(),
        );

        // Make handles available to other modules (e.g., memory_api)
        let _ = MEMORY_ITEMS_PINNED_GAUGE.set(pinned_g);
        let _ = MEMORY_ITEMS_UNPINNED_GAUGE.set(unpinned_g);
        let _ = MEMORY_EVICTIONS_EXPIRED.set(expired_c.clone());
        let _ = MEMORY_EVICTIONS_MANUAL.set(manual_c.clone());

        // Spawn polling task to refresh gauges and push deltas of expired evictions.
        tokio::spawn(async move {
            use std::time::Duration;
            let mut last_expired = memory::expired_evictions_total();
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                // Snapshot
                if let Ok(stats) = memory::global().stats() {
                    if let Some(g) = MEMORY_ITEMS_PINNED_GAUGE.get() {
                        g.set(stats.pinned as i64);
                    }
                    if let Some(g) = MEMORY_ITEMS_UNPINNED_GAUGE.get() {
                        g.set(stats.unpinned as i64);
                    }
                    if let Some(c) = MEMORY_EVICTIONS_EXPIRED.get() {
                        let now = stats.expired_evictions_total;
                        if now > last_expired {
                            c.inc_by(now - last_expired);
                            last_expired = now;
                        }
                    }
                }
            }
        });
    }

    (app, state)
}

fn core_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/healthz", get(healthz))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/ask", get(ask::ask_handler))
        .route("/assist", post(assist::assist_handler))
        .route("/v1/chat", post(chat::chat_handler))
}

fn memory_routes() -> Router<AppState> {
    Router::new()
        .route("/memory/get", post(memory_api::memory_get_handler))
        .route("/memory/set", post(memory_api::memory_set_handler))
        .route("/memory/evict", post(memory_api::memory_evict_handler))
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
    use crate::{ask::AskResponse, chat::ChatStubResponse};
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

        let res = app
            .clone()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let res = app
            .clone()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text_one = String::from_utf8(body.to_vec()).unwrap();

        let expected_health = r#"http_requests_total{method="GET",path="/health",status="200"} 1"#;
        assert!(
            text_one.contains(expected_health),
            "metrics missing labeled health counter:\n{text_one}"
        );

        let res = app
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text_two = String::from_utf8(body.to_vec()).unwrap();

        let expected_metrics =
            r#"http_requests_total{method="GET",path="/metrics",status="200"} 1"#;
        assert!(
            text_two.contains(expected_metrics),
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
    async fn ask_route_clamps_k_to_100() {
        let app = demo_app(false);

        let upsert_payload = json!({
            "doc_id": "ask-doc-large",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "ask-doc-large#0", "text": "Hallo Hauski", "embedding": []}
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
                    .uri("/ask?q=Hauski&k=250&ns=default")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ask_res.status(), StatusCode::OK);

        let body = ask_res.into_body().collect().await.unwrap().to_bytes();
        let response: AskResponse = from_slice(&body).unwrap();
        assert_eq!(response.k, 100);
        assert!(response.hits.len() <= 100);
    }

    #[tokio::test]
    async fn ask_route_clamps_k_to_minimum() {
        let app = demo_app(false);

        let upsert_payload = json!({
            "doc_id": "ask-doc-min",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "ask-doc-min#0", "text": "Hallo Hauski", "embedding": []}
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
                    .uri("/ask?q=Hauski&k=0&ns=default")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ask_res.status(), StatusCode::OK);

        let body = ask_res.into_body().collect().await.unwrap().to_bytes();
        let response: AskResponse = from_slice(&body).unwrap();
        assert_eq!(response.k, 1);
        assert!(response.hits.len() <= 1);
        assert!(
            response.hits.iter().any(|hit| hit.doc_id == "ask-doc-min"),
            "expected a hit with doc_id ask-doc-min"
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

        let expected_search =
            r#"http_requests_total{method="POST",path="/index/search",status="200"} 1"#;
        assert!(
            text.contains(expected_search),
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
    async fn healthz_ok() {
        let app = demo_app(false);
        let res = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn chat_stub_returns_service_unavailable_when_unconfigured() {
        let app = demo_app(false);
        let payload = json!({
            "messages": [
                {"role": "user", "content": "Hallo HausKI?"}
            ]
        });

        let res = app
            .oneshot(
                Request::post("/v1/chat")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let stub: ChatStubResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(stub.status, "unavailable");
    }

    #[tokio::test]
    async fn chat_stub_unavailable_payload_matches_message() {
        let app = demo_app(false);
        let payload = json!({
            "messages": [
                {"role": "user", "content": "Hallo HausKI?"}
            ]
        });

        let res = app
            .oneshot(
                Request::post("/v1/chat")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let stub: ChatStubResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(stub.status, "unavailable");
        assert_eq!(
            stub.message,
            "chat pipeline not wired yet, please configure HAUSKI_CHAT_UPSTREAM_URL"
        );
    }

    #[tokio::test]
    async fn safe_mode_flag_is_reflected_in_state() {
        let (_app, state) = demo_app_with_origin_and_flags(
            false,
            FeatureFlags {
                safe_mode: true,
                ..FeatureFlags::default()
            },
            HeaderValue::from_static("http://127.0.0.1:8080"),
        );
        assert!(state.safe_mode());
        assert!(state.flags().safe_mode);
    }
}
