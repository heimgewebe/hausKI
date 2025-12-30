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

        // 1. Setup: Insert open and closed preimages
        let key_open = "decision.preimage:open";
        let val_open = json!({ "status": "open", "context": "foo" });

        let key_closed = "decision.preimage:closed";
        let val_closed = json!({ "status": "closed", "context": "bar" });

        let key_flagged = "decision.preimage:flagged";
        let val_flagged = json!({ "status": "open", "needs_recheck": true });

        mem::global().set(key_open.to_string(), serde_json::to_vec(&val_open).unwrap(), mem::TtlUpdate::Set(300), Some(false)).await.unwrap();
        mem::global().set(key_closed.to_string(), serde_json::to_vec(&val_closed).unwrap(), mem::TtlUpdate::Set(300), Some(false)).await.unwrap();
        mem::global().set(key_flagged.to_string(), serde_json::to_vec(&val_flagged).unwrap(), mem::TtlUpdate::Set(300), Some(false)).await.unwrap();

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

        // 3. Assertion:

        // Open item should be flagged and have reason
        let item_open = mem::global().get(key_open.to_string()).await.unwrap().expect("open item missing");
        let json_open: serde_json::Value = serde_json::from_slice(&item_open.value).unwrap();
        assert_eq!(json_open["needs_recheck"], true, "Open item should be marked");
        assert!(json_open.get("recheck_reason").is_some(), "Reason should be added");
        assert_eq!(json_open["recheck_reason"]["type"], "knowledge.observatory.published.v1");

        // Closed item should be untouched
        let item_closed = mem::global().get(key_closed.to_string()).await.unwrap().expect("closed item missing");
        let json_closed: serde_json::Value = serde_json::from_slice(&item_closed.value).unwrap();
        assert!(json_closed.get("needs_recheck").is_none(), "Closed item should not be marked");

        // Already flagged item should be untouched (to be idempotent/not overwrite existing reason if we wanted, though current logic overwrites reason if not filtered out, but here we filter by !needs_recheck)
        // Wait, logic says: if is_open && !already_flagged. So it should skip.
        // Let's verify it skipped by checking if reason was added (it shouldn't be, because val_flagged didn't have it)
        let item_flagged = mem::global().get(key_flagged.to_string()).await.unwrap().expect("flagged item missing");
        let json_flagged: serde_json::Value = serde_json::from_slice(&item_flagged.value).unwrap();
        assert!(json_flagged.get("recheck_reason").is_none(), "Already flagged item should be skipped");

        // Cleanup
        mem::global().evict(key_open.to_string()).await.unwrap();
        mem::global().evict(key_closed.to_string()).await.unwrap();
        mem::global().evict(key_flagged.to_string()).await.unwrap();
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
