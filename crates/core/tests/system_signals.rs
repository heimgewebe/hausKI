use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hauski_core::{
    FeatureFlags, Limits, ModelsFile, RoutingPolicy,
    system::SystemSignals,
};
use http_body_util::BodyExt;
use axum::http::HeaderValue;
use tower::ServiceExt; // for oneshot

#[tokio::test]
async fn system_signals_returns_expected_keys() {
    // Setup minimal app state
    let limits = Limits {
        latency: hauski_core::Latency {
            llm_p95_ms: 400,
            index_topk20_ms: 60,
        },
        thermal: hauski_core::Thermal {
            gpu_max_c: 80,
            dgpu_power_w: 220,
        },
        asr: hauski_core::Asr { wer_max_pct: 10 },
    };
    let models = ModelsFile {
        models: vec![],
    };
    let routing = RoutingPolicy::default();
    let flags = FeatureFlags::default();
    let origin = HeaderValue::from_static("http://localhost");

    let (app, _state) = hauski_core::build_app_with_state(limits, models, routing, flags, false, origin);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/system/signals")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let signals: SystemSignals = serde_json::from_slice(&body).expect("Failed to deserialize SystemSignals");

    // Validate values are within expected ranges
    assert!(signals.cpu_load >= 0.0 && signals.cpu_load <= 100.0, "CPU load out of range");
    assert!(signals.memory_pressure >= 0.0 && signals.memory_pressure <= 100.0, "Memory pressure out of range");

    // We cannot assert true/false for GPU as it depends on the runner environment,
    // but the field must exist (which is guaranteed by type safety here).
}
