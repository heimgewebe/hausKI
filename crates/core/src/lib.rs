use axum::{http::StatusCode, response::IntoResponse, routing::get, Json, Router};
use prometheus_client::{
    encoding::{text::encode, EncodeLabelSet},
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod config;
pub use config::{load_limits, load_models, Limits, ModelsFile};

#[derive(Clone)]
pub struct AppState(Arc<AppStateInner>);

struct AppStateInner {
    limits: Limits,
    models: ModelsFile,
    http_requests_total: Family<HttpLabels, Counter<u64>>,
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

        let http_requests_total: Family<HttpLabels, Counter<u64>> = Family::default();
        registry.register(
            "http_requests_total",
            "Total HTTP requests by route",
            http_requests_total.clone(),
        );

        Self(Arc::new(AppStateInner {
            limits,
            models,
            http_requests_total,
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

    fn record_route(&self, route: &'static str) {
        self.0
            .http_requests_total
            .get_or_create(&HttpLabels { route })
            .inc();
    }

    fn encode_metrics(&self) -> Result<String, std::fmt::Error> {
        let mut body = String::new();
        encode(&mut body, &self.0.registry)?;
        Ok(body)
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet, Serialize, Deserialize)]
pub struct HttpLabels {
    route: &'static str,
}

async fn get_limits(state: AppState) -> Json<Limits> {
    Json(state.limits())
}

async fn get_models(state: AppState) -> Json<ModelsFile> {
    Json(state.models())
}

async fn health(state: AppState) -> &'static str {
    state.record_route("/health");
    "ok"
}

async fn metrics(state: AppState) -> impl IntoResponse {
    state.record_route("/metrics");
    match state.encode_metrics() {
        Ok(body) => (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            body,
        )
            .into_response(),
        Err(_err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/plain; version=0.0.4",
            )],
            "Internal server error".to_string(),
        )
            .into_response(),
    }
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
            text.contains(r#"http_requests_total_total{route="/health"}"#),
            "metrics missing labeled counter:\n{text}"
        );
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
