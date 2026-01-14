use axum::http::HeaderValue;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hauski_core::{system::SystemSignals, FeatureFlags, Limits, ModelsFile, RoutingPolicy};
use http_body_util::BodyExt;
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

    // Validate metric values are within expected ranges
    assert!(
        signals.cpu_load >= 0.0 && signals.cpu_load <= 100.0,
        "CPU load out of range"
    );
    assert!(
        signals.memory_pressure >= 0.0 && signals.memory_pressure <= 100.0,
        "Memory pressure out of range"
    );

    // Validate that values are finite (not NaN or Inf)
    assert!(signals.cpu_load.is_finite(), "CPU load is not finite");
    assert!(
        signals.memory_pressure.is_finite(),
        "Memory pressure is not finite"
    );

    // Validate contract-required timestamp field
    // occurred_at must be a valid RFC3339 timestamp (ensured by DateTime<Utc> type)
    // Check it's not in the future and not absurdly old (basic sanity check)
    let now = chrono::Utc::now();
    assert!(
        signals.occurred_at <= now,
        "occurred_at is in the future: {:?}",
        signals.occurred_at
    );

    // Ensure it's reasonably recent (within last 10 minutes to avoid CI flakes)
    let age = now.signed_duration_since(signals.occurred_at);
    assert!(
        age.num_seconds() < 600,
        "occurred_at is too old (>10 min): {:?}",
        signals.occurred_at
    );

    // Validate optional fields if present
    if let Some(ref source) = signals.source {
        assert!(!source.is_empty(), "source should not be empty if present");
    }
    if let Some(ref host) = signals.host {
        assert!(!host.is_empty(), "host should not be empty if present");
    }

    // We cannot assert true/false for GPU as it depends on the runner environment,
    // but the field must exist (which is guaranteed by type safety here).
}
