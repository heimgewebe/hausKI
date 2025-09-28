use axum::{extract::State, routing::get, Json, Router};
use prometheus_client::{
    encoding::text::encode,
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, sync::Arc};

mod config;
pub use config::{load_limits, load_models, Limits, ModelsFile};

#[derive(Clone)]
pub struct AppState {
    pub limits: Arc<Limits>,
    pub models: Arc<ModelsFile>,
    pub http_requests_total: Family<HttpLabels, Counter<u64>>,
    pub registry: Arc<Registry>,
    pub expose_config: bool,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpLabels {
    route: Cow<'static, str>,
}

impl prometheus_client::encoding::text::EncodeLabelSet for HttpLabels {}

async fn get_limits(State(state): State<AppState>) -> Json<Limits> {
    Json((*state.limits).clone())
}

async fn get_models(State(state): State<AppState>) -> Json<ModelsFile> {
    Json((*state.models).clone())
}

async fn health(State(state): State<AppState>) -> &'static str {
    state
        .http_requests_total
        .get_or_create(&HttpLabels {
            route: Cow::from("/health"),
        })
        .inc();
    "ok"
}

pub fn build_app(limits: Limits, models: ModelsFile, expose_config: bool) -> Router {
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

    let state = AppState {
        limits: Arc::new(limits),
        models: Arc::new(models),
        http_requests_total,
        registry: Arc::new(registry),
        expose_config,
    };

    let metrics_state = state.clone();
    let metrics = get(move || {
        let registry = metrics_state.registry.clone();
        async move {
            let mut body = String::new();
            encode(&mut body, &*registry).expect("encode metrics");
            body
        }
    });

    let mut app = Router::new()
        .route("/health", get(health))
        .route("/metrics", metrics)
        .with_state(state.clone());

    if state.expose_config {
        app = app
            .route("/config/limits", get(get_limits))
            .route("/config/models", get(get_models));
    }

    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{Request, StatusCode};
    use hyper::body::to_bytes;
    use tower::ServiceExt;

    fn demo_app(expose_config: bool) -> Router {
        let limits = Limits {
            latency: super::config::Latency {
                llm_p95_ms: 400,
                index_topk20_ms: 60,
            },
            thermal: super::config::Thermal {
                gpu_max_c: 80,
                dgpu_power_w: 220,
            },
            asr: super::config::Asr { wer_max_pct: 10 },
        };

        let models = ModelsFile {
            models: vec![super::config::ModelEntry {
                id: "llama3.1-8b-q4".into(),
                path: "/opt/models/llama3.1-8b-q4.gguf".into(),
                vram_min_gb: Some(6),
                canary: Some(false),
            }],
        };

        build_app(limits, models, expose_config)
    }

    #[tokio::test]
    async fn health_ok_and_metrics_increment() {
        let app = demo_app(false);

        let response = app
            .clone()
            .oneshot(Request::get("/health").body(().into()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(Request::get("/metrics").body(().into()).unwrap())
            .await
            .unwrap();

        let body = to_bytes(response.into_body()).await.unwrap();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            text.contains(r#"http_requests_total{route="/health"}"#),
            "metrics missing labeled counter:\n{text}"
        );
    }

    #[tokio::test]
    async fn config_routes_hidden_by_default() {
        let app = demo_app(false);

        let response = app
            .clone()
            .oneshot(Request::get("/config/limits").body(().into()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = app
            .oneshot(Request::get("/config/models").body(().into()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn config_routes_visible_when_enabled() {
        let app = demo_app(true);

        let response = app
            .clone()
            .oneshot(Request::get("/config/limits").body(().into()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(Request::get("/config/models").body(().into()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
