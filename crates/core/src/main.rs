use hauski_core::{build_app, load_limits, load_models};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let limits_path =
        std::env::var("HAUSKI_LIMITS").unwrap_or_else(|_| "./policies/limits.yaml".into());
    let models_path =
        std::env::var("HAUSKI_MODELS").unwrap_or_else(|_| "./configs/models.yml".into());
    let expose_config = std::env::var("HAUSKI_EXPOSE_CONFIG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let app = build_app(
        load_limits(limits_path)?,
        load_models(models_path)?,
        expose_config,
    );

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::info!("listening on http://{addr}");
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
