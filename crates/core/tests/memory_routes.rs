use std::time::{SystemTime, UNIX_EPOCH};

use axum::{
    body::Body,
    http::{self, HeaderValue, Request, StatusCode},
};
use hauski_core::{build_app_with_state, FeatureFlags, Limits, ModelsFile, RoutingPolicy};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

#[tokio::test]
async fn memory_routes_available_without_expose_config() {
    let limits = Limits::default();
    let models = ModelsFile::default();
    let routing = RoutingPolicy::default();
    let flags = FeatureFlags::default();
    let allowed_origin = HeaderValue::from_static("*");
    let (app, _state) = build_app_with_state(limits, models, routing, flags, false, allowed_origin);

    let key = format!(
        "memory-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos()
    );
    let set_payload = json!({ "key": key, "value": "hello" });

    let set_response = app
        .clone()
        .oneshot(
            Request::post("/memory/set")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(set_payload.to_string()))
                .expect("failed to build request"),
        )
        .await
        .expect("set request failed");

    assert_eq!(set_response.status(), StatusCode::OK);

    let get_payload = json!({ "key": key });
    let get_response = app
        .clone()
        .oneshot(
            Request::post("/memory/get")
                .header(http::header::CONTENT_TYPE, "application/json")
                .body(Body::from(get_payload.to_string()))
                .expect("failed to build request"),
        )
        .await
        .expect("get request failed");

    assert_eq!(get_response.status(), StatusCode::OK);

    let body_bytes = get_response
        .into_body()
        .collect()
        .await
        .expect("body bytes")
        .to_bytes();
    let payload: Value = serde_json::from_slice(&body_bytes).expect("response json");
    assert_eq!(payload["value"], "hello");
}
