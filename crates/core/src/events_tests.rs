#[cfg(test)]
mod tests {
    use crate::{build_app_with_state, AppState, FeatureFlags, Limits, ModelsFile, RoutingPolicy};
    use axum::http::HeaderValue;
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
        Router,
    };
    use hauski_memory as mem;
    use serde_json::json;
    use tower::ServiceExt; // for oneshot

    // Helper to build a minimal app for testing
    fn test_app(flags: FeatureFlags) -> (Router, AppState) {
        let limits = Limits::default();
        let models = ModelsFile::default();
        let routing = RoutingPolicy::default();
        let allowed_origin = HeaderValue::from_static("http://127.0.0.1:8080");

        // Ensure memory is initialized (it might be already if running multiple tests, but init_default handles OnceCell)
        let _ = mem::init_default();

        let (app, state) =
            build_app_with_state(limits, models, routing, flags, false, allowed_origin);
        state.set_ready();
        (app, state)
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_auth_token_missing_config_returns_forbidden() {
        let flags = FeatureFlags::default(); // events_token is None
        let (app, _state) = test_app(flags);

        let event_payload = json!({
            "type": "some.event",
            "payload": { "url": "https://example.com" }
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

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_auth_token_configured_but_missing_header_returns_unauthorized() {
        let flags = FeatureFlags {
            events_token: Some("secret123".into()),
            ..FeatureFlags::default()
        };
        let (app, _state) = test_app(flags);

        let event_payload = json!({
            "type": "some.event",
            "payload": { "url": "https://example.com" }
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

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_auth_token_configured_and_invalid_header_returns_unauthorized() {
        let flags = FeatureFlags {
            events_token: Some("secret123".into()),
            ..FeatureFlags::default()
        };
        let (app, _state) = test_app(flags);

        let event_payload = json!({
            "type": "some.event",
            "payload": { "url": "https://example.com" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "Bearer wrongtoken")
                    .body(Body::from(event_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_auth_token_valid_and_https_url_ok() {
        let flags = FeatureFlags {
            events_token: Some("secret123".into()),
            ..FeatureFlags::default()
        };
        let (app, _state) = test_app(flags);

        let event_payload = json!({
            "type": "some.event",
            "payload": { "url": "https://example.com" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "Bearer secret123")
                    .body(Body::from(event_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_https_enforcement_returns_bad_request() {
        let flags = FeatureFlags {
            events_token: Some("secret123".into()),
            ..FeatureFlags::default()
        };
        let (app, _state) = test_app(flags);

        let event_payload = json!({
            "type": "some.event",
            "payload": { "url": "http://insecure.com" }
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "Bearer secret123")
                    .body(Body::from(event_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_observatory_event_triggers_preimage_flagging() {
        let flags = FeatureFlags {
            events_token: Some("secret123".into()),
            ..FeatureFlags::default()
        };
        let (app, _state) = test_app(flags);

        // 1. Setup: Insert open and closed preimages
        let key_open = "decision.preimage:open";
        let val_open = json!({ "status": "open", "context": "foo" });

        mem::global()
            .set(
                key_open.to_string(),
                serde_json::to_vec(&val_open).unwrap(),
                mem::TtlUpdate::Set(300),
                Some(false),
            )
            .await
            .unwrap();

        // 2. Action: Send the event
        let event_payload = json!({
            "type": "knowledge.observatory.published.v1",
            "payload": {
                "url": "https://example.com/obs.json",
                "generated_at": "2023-10-27T10:00:00Z",
                "sha": "sha256:abcdef123456",
                "schema_ref": "https://schemas.heimgewebe.org/contracts/knowledge/observatory.schema.json"
            }
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/events")
                    .method(Method::POST)
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(header::AUTHORIZATION, "Bearer secret123")
                    .body(Body::from(event_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // 3. Assertion:
        let item_open = mem::global()
            .get(key_open.to_string())
            .await
            .unwrap()
            .expect("open item missing");
        let json_open: serde_json::Value = serde_json::from_slice(&item_open.value).unwrap();
        assert_eq!(
            json_open["needs_recheck"], true,
            "Open item should be marked"
        );

        let reason = &json_open["recheck_reason"];
        assert_eq!(reason["sha"], "sha256:abcdef123456");
        assert_eq!(
            reason["schema_ref"],
            "https://schemas.heimgewebe.org/contracts/knowledge/observatory.schema.json"
        );

        // Cleanup
        mem::global().evict(key_open.to_string()).await.unwrap();
    }
}
