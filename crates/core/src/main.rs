use axum::{routing::get, Router};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use prometheus_client::{
    encoding::text::encode,
    metrics::{counter::Counter, family::Family, gauge::Gauge},
    registry::Registry,
};

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
        .route("/metrics", metrics);

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on http://{addr}");

    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
