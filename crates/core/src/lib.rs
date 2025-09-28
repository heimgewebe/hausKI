use axum::{
    http::{header::CONTENT_TYPE, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use prometheus_client::{
    encoding::{text::encode, EncodeLabel, EncodeLabelSet},
    metrics::{
        counter::Counter,
        family::Family,
        gauge::Gauge,
        histogram::{exponential_buckets, Histogram},
    },
    registry::Registry,
};
use std::{fmt, sync::Arc, time::Instant};

mod config;
pub use config::{load_limits, load_models, Limits, ModelsFile};

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

struct AppStateInner {
    limits: Limits,
    models: ModelsFile,
    http_requests: Family<HttpLabels, Counter<u64>>,
    http_latency: Family<HttpLabels, Histogram>,
    registry: Registry,
    /// Controls whether configuration endpoints are exposed.
    ///
    /// WARNING: Enabling this may expose sensitive configuration information.
    /// Only set to `true` if you understand the security implications.
    expose_config: bool,
}

impl AppState {
    fn new(limits: Limits, models: ModelsFile, expose_config: bool) -> Self {
        let mut registry = Registry::default();

        let build_info = Family::<(), Gauge>::default();
        build_info.get_or_create(&()).set(1);
        registry.register("hauski_build_info", "static 1", build_info);

        let http_requests: Family<HttpLabels, Counter<u64>> = Family::default();
        registry.register(
            "http_requests_total",
            "Total number of HTTP requests received",
            http_requests.clone(),
        );

        let buckets = exponential_buckets(0.005, 2.0, 14);
        let http_latency: Family<HttpLabels, Histogram> = Family::new_with_constructor({
            let buckets = buckets.clone();
            move || Histogram::new(buckets.clone())
        });
        registry.register(
            "http_request_duration_seconds",
            "HTTP request duration",
            http_latency.clone(),
        );

        Self(Arc::new(AppStateInner {
            limits,
            models,
            http_requests,
            http_latency,
            registry,
            expose_config,
        }))
    }

    fn limits(&self) -> Limits {
        self.0.limits.clone()
    }

    fn models(&self) -> ModelsFile {
        self.0.models.clone()
    }

    fn expose_config(&self) -> bool {
        self.0.expose_config
    }

    fn record_http_request(&self, method: Method, path: &'static str, status: StatusCode) {
        let labels = HttpLabels::new(method, path, status);
        self.0.http_requests.get_or_create(&labels).inc();
    }

    fn observe_http_latency(&self, labels: &HttpLabels, secs: f64) {
        self.0.http_latency.get_or_create(labels).observe(secs);
    }

    fn encode_metrics(&self) -> Result<String, std::fmt::Error> {
        let mut body = String::new();
        encode(&mut body, &self.0.registry)?;
        Ok(body)
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

async fn get_limits(state: AppState) -> Json<Limits> {
    let started = Instant::now();
    let status = StatusCode::OK;
    let response = Json(state.limits());
    state.record_http_request(Method::GET, "/config/limits", status);
    state.observe_http_latency(
        &HttpLabels::new(Method::GET, "/config/limits", status),
        started.elapsed().as_secs_f64(),
    );
    response
}

async fn get_models(state: AppState) -> Json<ModelsFile> {
    let started = Instant::now();
    let status = StatusCode::OK;
    let response = Json(state.models());
    state.record_http_request(Method::GET, "/config/models", status);
    state.observe_http_latency(
        &HttpLabels::new(Method::GET, "/config/models", status),
        started.elapsed().as_secs_f64(),
    );
    response
}

async fn health(state: AppState) -> &'static str {
    let started = Instant::now();
    let status = StatusCode::OK;
    state.record_http_request(Method::GET, "/health", status);
    state.observe_http_latency(
        &HttpLabels::new(Method::GET, "/health", status),
        started.elapsed().as_secs_f64(),
    );
    "ok"
}

async fn metrics(state: AppState) -> impl IntoResponse {
    let started = Instant::now();
    let (status, response) = match state.encode_metrics() {
        Ok(body) => (
            StatusCode::OK,
            (
                StatusCode::OK,
                [(CONTENT_TYPE, "text/plain; version=0.0.4")],
                body,
            )
                .into_response(),
        ),
        Err(_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(CONTENT_TYPE, "text/plain; version=0.0.4")],
                "Internal server error".to_string(),
            )
                .into_response(),
        ),
    };

    state.record_http_request(Method::GET, "/metrics", status);
    state.observe_http_latency(
        &HttpLabels::new(Method::GET, "/metrics", status),
        started.elapsed().as_secs_f64(),
    );

    response
}

pub fn build_app(limits: Limits, models: ModelsFile, expose_config: bool) -> Router {
    let state = AppState::new(limits, models, expose_config);

    let health_state = state.clone();
    let metrics_state = state.clone();

    let mut app = Router::new()
        .route(
            "/health",
            get(move || {
                let state = health_state.clone();
                async move { health(state).await }
            }),
        )
        .route(
            "/metrics",
            get(move || {
                let state = metrics_state.clone();
                async move { metrics(state).await }
            }),
        );

    if state.expose_config() {
        let limits_state = state.clone();
        let models_state = state.clone();
        app = app
            .route(
                "/config/limits",
                get(move || {
                    let state = limits_state.clone();
                    async move { get_limits(state).await }
                }),
            )
            .route(
                "/config/models",
                get(move || {
                    let state = models_state.clone();
                    async move { get_models(state).await }
                }),
            );
    }

    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn demo_app(expose: bool) -> axum::Router {
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
        build_app(limits, models, expose)
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
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            text.contains(r#"http_requests_total{method="GET",path="/health",status="200"}"#),
            "metrics missing labeled health counter:\n{text}"
        );
        assert!(
            text.contains(r#"http_requests_total{method="GET",path="/metrics",status="200"}"#),
            "metrics missing labeled metrics counter:\n{text}"
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
            .oneshot(Request::get("/config/models").body(Body::empty()).unwrap())
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
            .oneshot(Request::get("/config/models").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
