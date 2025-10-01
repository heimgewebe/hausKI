use axum::http::HeaderValue;
use hauski_core::{build_app_with_state, load_flags, load_limits, load_models, load_routing};
use std::{env, net::SocketAddr};
use tokio::{net::TcpListener, signal};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer())
        .init();

    let limits_path = env::var("HAUSKI_LIMITS").unwrap_or_else(|_| "./policies/limits.yaml".into());
    let models_path = env::var("HAUSKI_MODELS").unwrap_or_else(|_| "./configs/models.yml".into());
    let routing_path =
        env::var("HAUSKI_ROUTING").unwrap_or_else(|_| "./policies/routing.yaml".into());
    let flags_path = env::var("HAUSKI_FLAGS").unwrap_or_else(|_| "./configs/flags.yaml".into());
    let expose_config = env::var("HAUSKI_EXPOSE_CONFIG")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let allowed_origin =
        env::var("HAUSKI_ALLOWED_ORIGIN").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let allowed_origin_header = HeaderValue::from_str(&allowed_origin).map_err(|e| {
        anyhow::anyhow!("invalid HAUSKI_ALLOWED_ORIGIN '{}': {}", allowed_origin, e)
    })?;

    let (app, state) = build_app_with_state(
        load_limits(limits_path)?,
        load_models(models_path)?,
        load_routing(routing_path)?,
        load_flags(flags_path)?,
        expose_config,
        allowed_origin_header,
    );

    let addr = resolve_bind_addr(expose_config)?;
    tracing::info!(%addr, expose_config, "starting server");
    let listener = TcpListener::bind(addr).await?;
    state.set_ready();
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Resolve bind address with safe defaults:
/// - Default: 127.0.0.1:8080 (loopback only)
/// - Respect $HAUSKI_BIND if set (e.g. "0.0.0.0:8080")
/// - If EXPOSE_CONFIG=true, enforce loopback-only
fn resolve_bind_addr(expose_config: bool) -> anyhow::Result<SocketAddr> {
    let bind = env::var("HAUSKI_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let addr: SocketAddr = bind
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid HAUSKI_BIND '{}': {}", bind, e))?;
    let is_loopback = match addr.ip() {
        std::net::IpAddr::V4(v4) => v4.is_loopback(),
        std::net::IpAddr::V6(v6) => v6.is_loopback(),
    };
    if expose_config && !is_loopback {
        anyhow::bail!(
            "HAUSKI_EXPOSE_CONFIG requires loopback bind; set HAUSKI_BIND=127.0.0.1:<port>"
        );
    }
    if !expose_config && !is_loopback {
        tracing::warn!(
            "binding to non-loopback address ({}); EXPOSE_CONFIG is false",
            addr
        );
    }
    Ok(addr)
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received");
}

#[cfg(test)]
mod tests_bind {
    use super::*;
    use std::env;

    #[serial_test::serial]
    #[test]
    fn default_is_loopback_127_8080() {
        env::remove_var("HAUSKI_BIND");
        let addr = resolve_bind_addr(false).unwrap();
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 8080);
    }

    #[serial_test::serial]
    #[test]
    fn expose_requires_loopback() {
        env::set_var("HAUSKI_BIND", "0.0.0.0:8080");
        let err = resolve_bind_addr(true).unwrap_err().to_string();
        assert!(err.contains("requires loopback"));
        env::set_var("HAUSKI_BIND", "127.0.0.1:8080");
        let ok = resolve_bind_addr(true).unwrap();
        assert!(ok.ip().is_loopback());
    }
}
