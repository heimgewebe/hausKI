use std::{net::SocketAddr, sync::Arc};

use axum::{routing::post, Json, Router};
use hauski_policy_api::{
    heimlern::{Context, RemindBandit},
    utils::events::write_event_line,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::RwLock;

#[derive(Clone)]
struct AppState {
    policy: Arc<RwLock<RemindBandit>>,
}

#[derive(Deserialize)]
struct DecideReq {
    kind: String,
    #[serde(default = "default_features")]
    features: Value,
}

#[derive(Deserialize)]
struct FeedbackReq {
    kind: String,
    action: String,
    reward: f32,
    #[serde(default = "default_features")]
    features: Value,
}

#[derive(Serialize)]
struct DecideResp {
    action: String,
    score: f32,
    why: String,
    context: Value,
}

fn default_features() -> Value {
    json!({})
}

#[tokio::main]
async fn main() {
    let state = AppState {
        policy: Arc::new(RwLock::new(RemindBandit)),
    };

    let app = Router::new()
        .route(
            "/v1/policy/decide",
            post({
                let state = state.clone();
                move |Json(req): Json<DecideReq>| {
                    let state = state.clone();
                    async move {
                        let ctx = Context {
                            kind: req.kind,
                            features: req.features,
                        };
                        let mut policy = state.policy.write().await;
                        let decision = policy.decide(&ctx);
                        let resp = DecideResp {
                            action: decision.action.clone(),
                            score: decision.score,
                            why: decision.why.clone(),
                            context: decision.context.unwrap_or_else(|| json!({})),
                        };
                        let payload = json!({
                            "decision": serde_json::to_value(&resp).unwrap_or_else(|_| json!({})),
                            "ts": chrono::Utc::now().to_rfc3339(),
                        });
                        write_event_line("policy.decide", &payload);
                        Json(resp)
                    }
                }
            }),
        )
        .route(
            "/v1/policy/feedback",
            post({
                let state = state.clone();
                move |Json(req): Json<FeedbackReq>| {
                    let state = state.clone();
                    async move {
                        let ctx = Context {
                            kind: req.kind,
                            features: req.features,
                        };
                        let mut policy = state.policy.write().await;
                        policy.feedback(&ctx, &req.action, req.reward);
                        let payload = json!({
                            "action": req.action,
                            "reward": req.reward,
                            "ts": chrono::Utc::now().to_rfc3339(),
                        });
                        write_event_line("policy.feedback", &payload);
                        Json(json!({ "ok": true }))
                    }
                }
            }),
        );

    let addr: SocketAddr = std::env::var("HAUSKI_POLICY_ADDR")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 8779)));

    println!("policy api on http://{addr}");

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind policy api listener on {addr}: {e}");
            std::process::exit(1);
        }
    };

    if let Err(e) = axum::serve(listener, app.into_make_service()).await {
        eprintln!("Policy api server failed: {e}");
        std::process::exit(1);
    }
}
