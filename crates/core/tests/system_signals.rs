use axum::http::HeaderValue;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hauski_core::{system::SystemSignals, FeatureFlags, Limits, ModelsFile, RoutingPolicy};
use http_body_util::BodyExt;
use tower::ServiceExt; // for oneshot

/// Test that /system/signals endpoint returns valid data according to the contract.
///
/// Contract: hauski.system.signals.v1
/// Schema: docs/contracts/hauski/system.signals.v1.schema.json
///
/// This test validates:
/// - HTTP 200 response
/// - All required fields present (cpu_load, memory_pressure, gpu_available)
/// - cpu_load in range [0.0, 100.0]
/// - memory_pressure in range [0.0, 100.0]
/// - gpu_available is a boolean (type-safe, but field must exist)
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
    let models = ModelsFile { models: vec![] };
    let routing = RoutingPolicy::default();
    let flags = FeatureFlags::default();
    let origin = HeaderValue::from_static("http://localhost");

    let (app, _state) =
        hauski_core::build_app_with_state(limits, models, routing, flags, false, origin);

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
    let signals: SystemSignals =
        serde_json::from_slice(&body).expect("Failed to deserialize SystemSignals");

    // Validate contract constraints:
    // cpu_load and memory_pressure must be in [0.0, 100.0]
    assert!(
        signals.cpu_load >= 0.0 && signals.cpu_load <= 100.0,
        "CPU load out of range: {}",
        signals.cpu_load
    );
    assert!(
        signals.memory_pressure >= 0.0 && signals.memory_pressure <= 100.0,
        "Memory pressure out of range: {}",
        signals.memory_pressure
    );

    // gpu_available field must exist (type-safe via struct)
    // We cannot assert true/false as it depends on the runner environment
    // (CI, WSL, Codespaces may not have NVIDIA GPU)
    let _ = signals.gpu_available; // Ensure field exists
}

/// Test that JSON deserialization enforces contract constraints.
/// This validates that the schema's required fields and types are correct.
#[test]
fn contract_validation_rejects_invalid_json() {
    // Missing required field
    let invalid = r#"{"cpu_load": 50.0, "memory_pressure": 60.0}"#;
    assert!(
        serde_json::from_str::<SystemSignals>(invalid).is_err(),
        "Should reject JSON missing gpu_available"
    );

    // Extra field (additionalProperties: false in schema)
    // Note: Rust serde with deny_unknown_fields would enforce this
    // The current struct doesn't enforce it, but the schema does.
    // For full validation, we'd need to validate against the JSON schema file.

    // Valid example
    let valid = r#"{"cpu_load": 50.0, "memory_pressure": 60.0, "gpu_available": true}"#;
    let signals: SystemSignals = serde_json::from_str(valid).expect("Should parse valid JSON");
    assert_eq!(signals.cpu_load, 50.0);
    assert_eq!(signals.memory_pressure, 60.0);
    assert!(signals.gpu_available);
}
