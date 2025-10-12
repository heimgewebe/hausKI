use std::{net::SocketAddr, sync::Arc};

use axum::{routing::post, Json, Router};
use hauski_policy_api::utils::events::write_event_line;
use heimlern_bandits::RemindBandit;
use heimlern_core::{Context, Policy};
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
    features: Value,
}

#[derive(Deserialize)]
struct FeedbackReq {
    kind: String,
    action: String,
    reward: f32,
    features: Option<Value>,
}

#[derive(Serialize)]
struct DecideResp {
    action: String,
    score: f32,
    why: String,
    context: Value,
}

#[tokio::main]
async fn main() {
    let state = AppState {
        policy: Arc::new(RwLock::new(RemindBandit::default())),
    };

    let app = Router::new()
        .route(
            "/v1/policy/decide",
            post({
                let st = state.clone();
                move |Json(req): Json<DecideReq>| {
                    let st = st.clone();
                    async move {
                        let ctx = Context {
                            kind: req.kind,
                            features: req.features,
                        };
                        let mut pol = st.policy.write().await;
                        let decision = pol.decide(&ctx);
                        let resp = DecideResp {
                            action: decision.action.clone(),
                            score: decision.score,
                            why: decision.why.clone(),
                            context: decision.context.unwrap_or_else(|| json!({})),
                        };
                        write_event_line(
                            "policy.decide",
                            &json!({
                                "decision": resp,
                                "ts": chrono::Utc::now().to_rfc3339(),
                            }),
                        );
                        Json(resp)
                    }
                }
            }),
        )
        .route(
            "/v1/policy/feedback",
            post({
                let st = state.clone();
                move |Json(req): Json<FeedbackReq>| {
                    let st = st.clone();
                    async move {
                        let ctx = Context {
                            kind: req.kind,
                            features: req.features.unwrap_or_else(|| json!({})),
                        };
                        let mut pol = st.policy.write().await;
                        pol.feedback(&ctx, &req.action, req.reward);
                        write_event_line(
                            "policy.feedback",
                            &json!({
                                "action": req.action,
                                "reward": req.reward,
                            }),
                        );
                        Json(json!({ "ok": true }))
                    }
                }
            }),
        );

    let addr = SocketAddr::from(([127, 0, 0, 1], 8779));
    println!("policy api on http://{addr}");
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
