use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use policy::remind_bandit::{DecisionContext, RemindBandit};
use policy::utils::events::write_event_line;
use policy::utils::policy_store::{load_snapshot, save_snapshot};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::RwLock;
use tracing::{error, info};

#[derive(Clone)]
struct AppState {
    policy: Arc<RwLock<RemindBandit>>,
}

#[derive(Debug, Deserialize)]
struct DecideRequest {
    kind: String,
    #[serde(default)]
    features: Value,
}

#[derive(Debug, Deserialize)]
struct FeedbackRequest {
    kind: String,
    action: String,
    reward: f32,
    #[serde(default)]
    features: Value,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let mut policy = RemindBandit::default();
    if let Ok(Some(snapshot)) = load_snapshot("remind_bandit") {
        policy.load(snapshot);
    }

    let state = AppState {
        policy: Arc::new(RwLock::new(policy)),
    };

    let app = Router::new()
        .route("/ready", get(ready_handler))
        .route("/v1/policy/decide", post(decide_handler))
        .route("/v1/policy/feedback", post(feedback_handler))
        .with_state(state.clone());

    let addr: SocketAddr = std::env::var("HAUSKI_POLICY_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8779".to_string())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "policy api listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(state))
        .await?;

    Ok(())
}

async fn ready_handler() -> &'static str {
    "ok"
}

async fn decide_handler(
    State(state): State<AppState>,
    Json(req): Json<DecideRequest>,
) -> Json<Value> {
    let DecideRequest { kind, features } = req;
    let ctx = DecisionContext {
        kind: kind.clone(),
        features: features.clone(),
    };

    let decision = {
        let mut guard = state.policy.write().await;
        guard.decide(&ctx)
    };

    let action = decision.action;
    let parameters = decision.parameters;

    let response = json!({
        "kind": kind.clone(),
        "action": action.clone(),
        "parameters": parameters,
    });
    write_event_line(
        "policy.decide",
        &json!({
            "kind": ctx.kind,
            "features": features,
            "action": action,
        }),
    );

    Json(response)
}

async fn feedback_handler(
    State(state): State<AppState>,
    Json(req): Json<FeedbackRequest>,
) -> Json<Value> {
    let FeedbackRequest {
        kind,
        action,
        reward,
        features,
    } = req;

    let ctx = DecisionContext {
        kind: kind.clone(),
        features: features.clone(),
    };

    let snapshot = {
        let mut guard = state.policy.write().await;
        guard.feedback(&ctx, &action, reward);
        guard.snapshot()
    };

    if let Err(err) = save_snapshot("remind_bandit", &snapshot) {
        error!(%err, "failed to persist policy snapshot");
    }

    write_event_line(
        "policy.feedback",
        &json!({
            "kind": kind,
            "action": action,
            "reward": reward,
            "features": features,
        }),
    );

    Json(json!({"status": "ok"}))
}

async fn shutdown_signal(state: AppState) {
    if let Err(err) = tokio::signal::ctrl_c().await {
        error!(%err, "failed to listen for shutdown signal");
        return;
    }
    info!("shutdown signal received");

    let snapshot = {
        let guard = state.policy.read().await;
        guard.snapshot()
    };

    if let Err(err) = save_snapshot("remind_bandit", &snapshot) {
        error!(%err, "failed to persist policy snapshot on shutdown");
    }
}
