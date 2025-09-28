use axum::{
    extract::State,
    http::{header::CONTENT_TYPE, Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use prometheus_client::{
    encoding::{EncodeLabel, EncodeLabelSet},
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};
use std::{fmt, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod config;
use crate::config::{load_limits, load_models, Limits, ModelsFile};

#[derive(Clone)]
struct AppState {
    limits: Arc<Limits>,
    models: Arc<ModelsFile>,
    registry: Arc<Registry>,
    http_requests_total: Family<HttpLabels, Counter>,
}

async fn get_limits(State(state): State<AppState>) -> Json<Limits> {
    state.record_http_request(Method::GET, "/config/limits", StatusCode::OK);
    Json((*state.limits).clone())
}

async fn get_models(State(state): State<AppState>) -> Json<ModelsFile> {
    state.record_http_request(Method::GET, "/config/models", StatusCode::OK);
    Json((*state.models).clone())
}

async fn health(State(state): State<AppState>) -> &'static str {
    state.record_http_request(Method::GET, "/health", StatusCode::OK);
    "ok"
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let mut body = String::new();
    match prometheus_client::encoding::text::encode(&mut body, &state.registry) {
        Ok(()) => {
            state.record_http_request(Method::GET, "/metrics", StatusCode::OK);
            (
                StatusCode::OK,
                [(CONTENT_TYPE, "text/plain; version=0.0.4")],
                body,
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to encode metrics: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct HttpLabels {
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

impl AppState {
    fn record_http_request(&self, method: Method, path: &'static str, status: StatusCode) {
        let labels = HttpLabels::new(method, path, status);
        self.http_requests_total.get_or_create(&labels).inc();
    }
}

#[allow(clippy::explicit_auto_deref)]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let mut registry = Registry::default();

    let build_info = Family::<(), Gauge>::default();
    build_info.get_or_create(&()).set(1);
    registry.register("hauski_build_info", "static 1", build_info);

    let http_requests_total = Family::<HttpLabels, Counter>::default();
    registry.register(
        "http_requests_total",
        "Total number of HTTP requests received",
        http_requests_total.clone(),
    );

    let limits_path =
        std::env::var("HAUSKI_LIMITS").unwrap_or_else(|_| "./policies/limits.yaml".to_string());
    let models_path =
        std::env::var("HAUSKI_MODELS").unwrap_or_else(|_| "./configs/models.yml".to_string());
    let limits = Arc::new(load_limits(&limits_path)?);
    let models = Arc::new(load_models(&models_path)?);

    let app_state = AppState {
        limits,
        models,
        registry: Arc::new(registry),
        http_requests_total,
    };

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/config/limits", get(get_limits))
        .route("/config/models", get(get_models))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
