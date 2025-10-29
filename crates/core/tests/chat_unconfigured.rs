use axum::body::Body;
use hauski_core::{build_app_with_state, FeatureFlags, Limits, ModelsFile, RoutingPolicy};
use http::{Request, StatusCode};
use hyper::body::to_bytes;
use serde_json::{json, Value};
use tower::ServiceExt;

fn default_app() -> axum::Router {
    for key in [
        "HAUSKI_CHAT_UPSTREAM_URL",
        "CHAT_UPSTREAM_URL",
        "HAUSKI_CHAT_MODEL",
    ] {
        std::env::remove_var(key);
    }

    let limits = Limits::default();
    let models = ModelsFile::default();
    let routing = RoutingPolicy::default();
    let flags = FeatureFlags::default();
    let allowed_origin = http::HeaderValue::from_static("*");
    let (app, _state) = build_app_with_state(limits, models, routing, flags, false, allowed_origin);
    app
}

#[tokio::test]
async fn chat_returns_503_when_unconfigured() {
    let app = default_app();
    let payload = json!({
        "messages": [
            {"role": "user", "content": "Ping?"}
        ]
    });

    let response = app
        .clone()
        .oneshot(
            Request::post("/v1/chat")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .expect("failed to build request"),
        )
        .await
        .expect("request failed");

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let retry_after = response
        .headers()
        .get(http::header::RETRY_AFTER)
        .expect("missing Retry-After header")
        .to_str()
        .expect("Retry-After not valid utf8");
    assert_eq!(retry_after, "30");

    let body_bytes = to_bytes(response.into_body()).await.expect("body bytes");
    let stub: Value = serde_json::from_slice(&body_bytes).expect("stub response");
    assert_eq!(stub["status"], "unavailable");
}
