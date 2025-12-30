#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
        Router,
    };
    use tower::ServiceExt; // for oneshot
    use serde_json::json;
    use crate::{build_app_with_state, AppState, FeatureFlags, Limits, ModelsFile, RoutingPolicy};
    use axum::http::HeaderValue;
    use std::sync::Arc;
    use hauski_memory as mem;

    // Helper to build a minimal app for testing
    fn test_app() -> (Router, AppState) {
        let limits = Limits::default();
        let models = ModelsFile::default();
        let routing = RoutingPolicy::default();
        let flags = FeatureFlags::default();
        let allowed_origin = HeaderValue::from_static("http://127.0.0.1:8080");

        // Ensure memory is initialized (it might be already if running multiple tests, but init_default handles OnceCell)
        let _ = mem::init_default();

        let (app, state) = build_app_with_state(limits, models, routing, flags, false, allowed_origin);
        state.set_ready();
        (app, state)
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_observatory_event_triggers_preimage_flagging() {
        let (app, _state) = test_app();

        // 1. Setup: Insert a dummy "decision.preimage" into memory
        let key = "decision.preimage:123";
        let initial_value = json!({ "status": "open", "context": "foo" });
        mem::global()
            .set(
                key.to_string(),
                serde_json::to_vec(&initial_value).unwrap(),
                mem::TtlUpdate::Set(300),
                Some(false),
            )
            .await
            .expect("failed to set setup memory");

        // 2. Action: Send the event
        let event_payload = json!({
            "type": "knowledge.observatory.published.v1",
            "payload": {
                "url": "http://example.com/obs.json",
                "generated_at": "2023-10-27T10:00:00Z"
            }
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(event_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 3. Assertion: Check if "needs_recheck" was added to the memory item
        let item = mem::global().get(key.to_string()).await.unwrap().expect("item missing");
        let updated_json: serde_json::Value = serde_json::from_slice(&item.value).unwrap();

        assert_eq!(updated_json["needs_recheck"], true, "Item should be marked for recheck");
        assert_eq!(updated_json["status"], "open", "Original fields should be preserved");

        // Cleanup
        mem::global().evict(key.to_string()).await.unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_other_events_ignored() {
        let (app, _state) = test_app();

        let event_payload = json!({
            "type": "some.other.event",
            "payload": { "url": "..." }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(event_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
