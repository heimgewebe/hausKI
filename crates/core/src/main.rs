use axum::{extract::State, routing::get, Json, Router};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use prometheus_client::{
    encoding::text::encode,
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};

mod config;
use crate::config::{load_limits, load_models, Limits, ModelsFile};

#[derive(Clone)]
struct AppState {
    limits: Arc<Limits>,
    models: Arc<ModelsFile>,
}

async fn get_limits(State(state): State<AppState>) -> Json<Limits> {
    Json((*state.limits).clone())
}

async fn get_models(State(state): State<AppState>) -> Json<ModelsFile> {
    Json((*state.models).clone())
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

    let http_requests_total = Counter::<u64>::default();
    registry.register(
        "http_requests_total",
        "Total number of HTTP requests received",
        http_requests_total.clone(),
    );

    let registry = Arc::new(registry);
    let metrics_registry = registry.clone();

    let limits_path =
        std::env::var("HAUSKI_LIMITS").unwrap_or_else(|_| "./policies/limits.yaml".to_string());
    let models_path =
        std::env::var("HAUSKI_MODELS").unwrap_or_else(|_| "./configs/models.yml".to_string());
    let limits = Arc::new(load_limits(&limits_path)?);
    let models = Arc::new(load_models(&models_path)?);
    let app_state = AppState { limits, models };

    let metrics = get(move || {
        let registry = metrics_registry.clone();
        async move {
            let mut body = String::new();
            encode(&mut body, &*registry).expect("encode metrics");
            body
        }
    });

    let health_route = get(move || {
        let counter = http_requests_total.clone();
        async move {
            counter.inc();
            "ok"
        }
    });

    let app = Router::new()
        .route("/health", health_route)
        .route("/metrics", metrics)
        .route("/config/limits", get(get_limits))
        .route("/config/models", get(get_models))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
