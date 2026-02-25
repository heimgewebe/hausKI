mod common;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::test_source_ref;
use hauski_indexd::{router, IndexState, PurgeStrategy, RetentionConfig};
use serde_json::json;
use std::sync::Arc;
use tower::ServiceExt;

/// Test the complete forget API endpoint with confirmation requirement
#[tokio::test]
async fn test_forget_api_requires_confirmation() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);
    let app = router().with_state(state.clone());

    // Add a document
    let upsert_payload = json!({
        "doc_id": "test-doc",
        "namespace": "test",
        "chunks": [
            {"chunk_id": "test-doc#0", "text": "Test content", "embedding": []}
        ],
        "meta": {},
        "source_ref": {
            "origin": "chronik",
            "id": "test-doc",
            "trust_level": "high"
        }
    });

    let _upsert_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/upsert")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(upsert_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Try to forget without confirmation (should fail)
    let forget_payload = json!({
        "filter": {
            "namespace": "test",
            "allow_namespace_wipe": true
        },
        "reason": "Test cleanup",
        "confirm": false,
        "dry_run": false
    });

    let forget_res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/forget")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(forget_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(forget_res.status(), StatusCode::BAD_REQUEST);

    // Now with confirmation (should succeed)
    let forget_confirmed = json!({
        "filter": {
            "namespace": "test",
            "allow_namespace_wipe": true
        },
        "reason": "Test cleanup",
        "confirm": true,
        "dry_run": false
    });

    let forget_ok = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/forget")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(forget_confirmed.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(forget_ok.status(), StatusCode::OK);
}

/// Test the retention config endpoint
#[tokio::test]
async fn test_retention_api_endpoint() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Set retention configs
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(3600),
                max_items: Some(1000),
                max_age_seconds: Some(86400),
                purge_strategy: Some(PurgeStrategy::Oldest),
            },
        )
        .await;

    let app = router().with_state(state.clone());

    // Get retention configs
    let res = app
        .oneshot(
            Request::builder()
                .uri("/retention")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert!(body.get("configs").is_some());
    let configs = body.get("configs").unwrap();
    assert!(configs.get("test").is_some());
}

/// Test the decay preview endpoint
#[tokio::test]
async fn test_decay_preview_api_endpoint() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Configure decay
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(3600),
                max_items: None,
                max_age_seconds: None,
                purge_strategy: None,
            },
        )
        .await;

    // Add documents
    for i in 1..=3 {
        state
            .upsert(hauski_indexd::UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "test".into(),
                chunks: vec![hauski_indexd::ChunkPayload {
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Content {}", i)),
                    text_lower: None,
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("chronik", "test-doc")),
            })
            .await
            .expect("upsert should succeed");
    }

    let app = router().with_state(state.clone());

    // Get decay preview
    let preview_payload = json!({
        "namespace": "test"
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/decay/preview")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(preview_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body.get("namespace").unwrap(), "test");
    assert_eq!(body.get("total_documents").unwrap(), 3);
    assert!(body.get("previews").unwrap().as_array().unwrap().len() == 3);
}

/// Test dry-run forget operation
#[tokio::test]
async fn test_forget_dry_run_api() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Add documents
    for i in 1..=3 {
        state
            .upsert(hauski_indexd::UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "test".into(),
                chunks: vec![hauski_indexd::ChunkPayload {
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Content {}", i)),
                    text_lower: None,
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("chronik", "test-doc")),
            })
            .await
            .expect("upsert should succeed");
    }

    let app = router().with_state(state.clone());

    // Dry-run forget
    let forget_dry = json!({
        "filter": {
            "namespace": "test",
            "allow_namespace_wipe": true
        },
        "reason": "Testing dry run",
        "confirm": false,
        "dry_run": true
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/forget")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(forget_dry.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(body.get("forgotten_count").unwrap(), 3);
    assert_eq!(body.get("dry_run").unwrap(), true);

    // Verify documents still exist
    let stats_res = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let stats_bytes = axum::body::to_bytes(stats_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats: serde_json::Value = serde_json::from_slice(&stats_bytes).unwrap();

    assert_eq!(stats.get("total_documents").unwrap(), 3);
}

/// Test search with time-decay applied
#[tokio::test]
async fn test_search_with_decay_applied() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Configure very aggressive decay
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(1), // 1 second
                max_items: None,
                max_age_seconds: None,
                purge_strategy: None,
            },
        )
        .await;

    // Add document
    state
        .upsert(hauski_indexd::UpsertRequest {
            doc_id: "test-doc".into(),
            namespace: "test".into(),
            chunks: vec![hauski_indexd::ChunkPayload {
                chunk_id: Some("test-doc#0".into()),
                text: Some("Testing decay in search".into()),
                text_lower: None,
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "test-doc")),
        })
        .await
        .expect("upsert should succeed");

    let app = router().with_state(state.clone());

    // Search immediately
    let search_payload1 = json!({
        "query": "testing",
        "k": 5,
        "namespace": "test"
    });

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/search")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(search_payload1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body1_bytes = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    let body1: serde_json::Value = serde_json::from_slice(&body1_bytes).unwrap();

    let matches1 = body1.get("matches").unwrap().as_array().unwrap();
    assert_eq!(matches1.len(), 1);
    let initial_score = matches1[0].get("score").unwrap().as_f64().unwrap();

    // Wait for decay
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Search again
    let search_payload2 = json!({
        "query": "testing",
        "k": 5,
        "namespace": "test"
    });

    let res2 = app
        .oneshot(
            Request::builder()
                .uri("/search")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(search_payload2.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    let body2_bytes = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    let body2: serde_json::Value = serde_json::from_slice(&body2_bytes).unwrap();

    let matches2 = body2.get("matches").unwrap().as_array().unwrap();
    assert_eq!(matches2.len(), 1);
    let decayed_score = matches2[0].get("score").unwrap().as_f64().unwrap();

    // Score should have decreased
    assert!(
        decayed_score < initial_score,
        "Decayed score {} should be less than initial {}",
        decayed_score,
        initial_score
    );
}

/// Test that forget API prevents unfiltered deletion
#[tokio::test]
async fn test_forget_api_prevents_unfiltered_deletion() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Add documents
    for i in 1..=3 {
        state
            .upsert(hauski_indexd::UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "test".into(),
                chunks: vec![hauski_indexd::ChunkPayload {
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Content {}", i)),
                    text_lower: None,
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("chronik", "test-doc")),
            })
            .await
            .expect("upsert should succeed");
    }

    let app = router().with_state(state.clone());

    // Try to forget without any content filters and without allow_namespace_wipe (should fail)
    let forget_no_filters = json!({
        "filter": {
            "namespace": "test"
            // No allow_namespace_wipe, no other filters
        },
        "reason": "Attempting unfiltered delete",
        "confirm": true,
        "dry_run": false
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/forget")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(forget_no_filters.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be rejected with BAD_REQUEST
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    // Check error message - should mention content filter requirement
    assert!(
        body.get("error").is_some(),
        "Response should contain 'error' field"
    );
    let error_msg = body.get("error").unwrap().as_str().unwrap();
    assert!(
        error_msg.contains("content filter"),
        "Error message should mention 'content filter', got: {}",
        error_msg
    );

    // Verify documents still exist
    let stats_res = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let stats_bytes = axum::body::to_bytes(stats_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats: serde_json::Value = serde_json::from_slice(&stats_bytes).unwrap();

    assert_eq!(stats.get("total_documents").unwrap(), 3);
}

/// Test critical security check: allow_namespace_wipe without namespace should be rejected
#[tokio::test]
async fn test_forget_api_prevents_global_wipe() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);
    let app = router().with_state(state.clone());

    // Add documents in multiple namespaces
    for ns in &["ns1", "ns2", "ns3"] {
        for i in 1..=2 {
            let upsert_payload = json!({
                "doc_id": format!("doc-{}", i),
                "namespace": ns,
                "chunks": [
                    {"chunk_id": format!("doc-{}#0", i), "text": format!("Content {} in {}", i, ns), "embedding": []}
                ],
                "meta": {},
                "source_ref": {
                    "origin": "chronik",
                    "id": format!("doc-{}", i),
                    "trust_level": "high"
                }
            });

            app.clone()
                .oneshot(
                    Request::builder()
                        .uri("/upsert")
                        .method("POST")
                        .header("content-type", "application/json")
                        .body(Body::from(upsert_payload.to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
        }
    }

    // Attempt: allow_namespace_wipe WITHOUT namespace (should fail)
    let forget_payload = json!({
        "filter": {
            "allow_namespace_wipe": true
            // namespace deliberately omitted
        },
        "reason": "Attempted global wipe",
        "confirm": true,
        "dry_run": false
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/forget")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(forget_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should be rejected with BAD_REQUEST
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let error_msg = body.get("error").unwrap().as_str().unwrap();
    assert!(
        error_msg.contains("allow_namespace_wipe") && error_msg.contains("namespace"),
        "Error should mention allow_namespace_wipe requires namespace, got: {}",
        error_msg
    );

    // Verify ALL documents still exist in ALL namespaces
    let stats_res = app
        .oneshot(
            Request::builder()
                .uri("/stats")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let stats_bytes = axum::body::to_bytes(stats_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let stats: serde_json::Value = serde_json::from_slice(&stats_bytes).unwrap();

    // Should have 6 total documents (3 namespaces × 2 docs each)
    assert_eq!(stats.get("total_documents").unwrap(), 6);
}

/// Test that upsert without source_ref returns 422 error instead of panicking
#[tokio::test]
async fn test_upsert_missing_source_ref_returns_error() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);
    let app = router().with_state(state.clone());

    // Try to upsert without source_ref
    let upsert_payload = json!({
        "doc_id": "test-doc",
        "namespace": "test",
        "chunks": [
            {"chunk_id": "test-doc#0", "text": "Test content", "embedding": []}
        ],
        "meta": {}
        // Missing source_ref
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/upsert")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(upsert_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Should return 422 Unprocessable Entity
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Check error response structure
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    let error: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    assert_eq!(error.get("code").unwrap(), "missing_source_ref");
    assert!(error.get("error").is_some());
    assert!(error.get("details").is_some());
}
