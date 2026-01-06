mod common;
use common::test_source_ref;

use hauski_indexd::{ChunkPayload, IndexState, SearchRequest, UpsertRequest};
use serde_json::json;
use std::sync::Arc;

/// Test that trust level affects search ranking
#[tokio::test]
async fn test_trust_weighting_affects_ranking() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Insert three documents with identical content but different trust levels
    // High trust (weight: 1.0)
    state
        .upsert(UpsertRequest {
            doc_id: "doc-high-trust".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-high-trust#0".into()),
                text: Some("Important security update information".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "high-trust-doc")),
        })
        .await
        .expect("upsert should succeed");

    // Medium trust (weight: 0.7)
    state
        .upsert(UpsertRequest {
            doc_id: "doc-medium-trust".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-medium-trust#0".into()),
                text: Some("Important security update information".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("osctx", "medium-trust-doc")),
        })
        .await
        .expect("upsert should succeed");

    // Low trust (weight: 0.3)
    state
        .upsert(UpsertRequest {
            doc_id: "doc-low-trust".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-low-trust#0".into()),
                text: Some("Important security update information".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("external", "low-trust-doc")),
        })
        .await
        .expect("upsert should succeed");

    // Search with include_weights to verify ranking
    let results = state
        .search(&SearchRequest {
            query: "security update".into(),
            k: Some(10),
            namespace: Some("default".into()),
            exclude_flags: Some(vec![]), // No filtering
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: true,
        })
        .await;

    assert_eq!(results.len(), 3, "Should return all three documents");

    // Verify ranking: high trust > medium trust > low trust
    assert_eq!(results[0].doc_id, "doc-high-trust");
    assert_eq!(results[1].doc_id, "doc-medium-trust");
    assert_eq!(results[2].doc_id, "doc-low-trust");

    // Verify scores are properly weighted
    assert!(
        results[0].score > results[1].score,
        "High trust should have higher score than medium trust"
    );
    assert!(
        results[1].score > results[2].score,
        "Medium trust should have higher score than low trust"
    );

    // Verify weight breakdown is included
    let weights_0 = results[0].weights.as_ref().unwrap();
    let weights_1 = results[1].weights.as_ref().unwrap();
    let weights_2 = results[2].weights.as_ref().unwrap();

    // All should have same similarity (identical text)
    assert!(
        (weights_0.similarity - weights_1.similarity).abs() < 0.01,
        "Similarity should be equal for identical text"
    );
    assert!(
        (weights_1.similarity - weights_2.similarity).abs() < 0.01,
        "Similarity should be equal for identical text"
    );

    // Trust weights should be as expected
    assert!(
        (weights_0.trust - 1.0).abs() < 0.01,
        "High trust weight should be 1.0"
    );
    assert!(
        (weights_1.trust - 0.7).abs() < 0.01,
        "Medium trust weight should be 0.7"
    );
    assert!(
        (weights_2.trust - 0.3).abs() < 0.01,
        "Low trust weight should be 0.3"
    );
}

/// Test that context profile affects namespace weighting
#[tokio::test]
async fn test_context_profile_weighting() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Insert documents in DIFFERENT namespaces to test context weighting
    // Context weighting is based on document.namespace, not metadata
    state
        .upsert(UpsertRequest {
            doc_id: "doc-chronik".into(),
            namespace: "chronik".into(), // Actually in chronik namespace
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-chronik#0".into()),
                text: Some("System event occurred".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "event-1")),
        })
        .await
        .expect("upsert should succeed");

    state
        .upsert(UpsertRequest {
            doc_id: "doc-code".into(),
            namespace: "code".into(), // Actually in code namespace
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-code#0".into()),
                text: Some("System event occurred".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("code", "code-file")),
        })
        .await
        .expect("upsert should succeed");

    state
        .upsert(UpsertRequest {
            doc_id: "doc-insights".into(),
            namespace: "insights".into(), // Actually in insights namespace
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-insights#0".into()),
                text: Some("System event occurred".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "insight-1")), // Same trust as doc-chronik
        })
        .await
        .expect("upsert should succeed");

    // Test incident_response profile on chronik namespace
    // chronik gets 1.2 boost, insights gets 0.8, code gets 0.5
    let results_chronik = state
        .search(&SearchRequest {
            query: "system event".into(),
            k: Some(10),
            namespace: Some("chronik".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: Some("incident_response".into()),
            include_weights: true,
        })
        .await;

    assert_eq!(results_chronik.len(), 1);
    let chronik_weights = results_chronik[0].weights.as_ref().unwrap();
    assert!(
        (chronik_weights.context - 1.2).abs() < 0.01,
        "Chronik namespace should have context weight 1.2 for incident_response"
    );

    // Test incident_response profile on code namespace
    let results_code = state
        .search(&SearchRequest {
            query: "system event".into(),
            k: Some(10),
            namespace: Some("code".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: Some("incident_response".into()),
            include_weights: true,
        })
        .await;

    assert_eq!(results_code.len(), 1);
    let code_weights = results_code[0].weights.as_ref().unwrap();
    assert!(
        (code_weights.context - 0.5).abs() < 0.01,
        "Code namespace should have context weight 0.5 for incident_response"
    );

    // Test code_analysis profile on code namespace
    let results_code_analysis = state
        .search(&SearchRequest {
            query: "system event".into(),
            k: Some(10),
            namespace: Some("code".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: Some("code_analysis".into()),
            include_weights: true,
        })
        .await;

    assert_eq!(results_code_analysis.len(), 1);
    let code_analysis_weights = results_code_analysis[0].weights.as_ref().unwrap();
    assert!(
        (code_analysis_weights.context - 1.2).abs() < 0.01,
        "Code namespace should have context weight 1.2 for code_analysis profile"
    );
}

/// Test combined weighting (trust + recency + context)
#[tokio::test]
async fn test_combined_weighting() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Insert document with high trust in code namespace
    state
        .upsert(UpsertRequest {
            doc_id: "doc-high-trust-code".into(),
            namespace: "code".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-high-trust-code#0".into()),
                text: Some("Function implementation details".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "verified-code")),
        })
        .await
        .expect("upsert should succeed");

    // Insert document with low trust in code namespace as well
    state
        .upsert(UpsertRequest {
            doc_id: "doc-low-trust-code".into(),
            namespace: "code".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-low-trust-code#0".into()),
                text: Some("Function implementation details".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("external", "external-doc")),
        })
        .await
        .expect("upsert should succeed");

    // Search with code_analysis profile in code namespace (context weight: 1.2)
    // High trust (1.0) × code (1.2) = 1.2
    // Low trust (0.3) × code (1.2) = 0.36
    let results = state
        .search(&SearchRequest {
            query: "function implementation".into(),
            k: Some(10),
            namespace: Some("code".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: Some("code_analysis".into()),
            include_weights: true,
        })
        .await;

    assert_eq!(results.len(), 2, "Should return both documents");

    // High trust code should rank first
    assert_eq!(
        results[0].doc_id, "doc-high-trust-code",
        "High trust code should rank first"
    );
    assert_eq!(
        results[1].doc_id, "doc-low-trust-code",
        "Low trust code should rank second"
    );

    // Verify final scores reflect combined weighting
    assert!(
        results[0].score > results[1].score,
        "Combined weighting should favor high trust"
    );

    // Verify context weights are applied correctly
    let weights_0 = results[0].weights.as_ref().unwrap();
    let weights_1 = results[1].weights.as_ref().unwrap();

    assert!(
        (weights_0.context - 1.2).abs() < 0.01,
        "Code namespace should have context weight 1.2 for code_analysis"
    );
    assert!(
        (weights_1.context - 1.2).abs() < 0.01,
        "Code namespace should have context weight 1.2 for code_analysis"
    );
}

/// Test that include_weights=false omits weight breakdown
#[tokio::test]
async fn test_weights_omitted_when_not_requested() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    state
        .upsert(UpsertRequest {
            doc_id: "doc-test".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-test#0".into()),
                text: Some("Test content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "test")),
        })
        .await
        .expect("upsert should succeed");

    let results = state
        .search(&SearchRequest {
            query: "test".into(),
            k: Some(10),
            namespace: Some("default".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false, // Explicitly don't include weights
        })
        .await;

    assert_eq!(results.len(), 1);
    assert!(
        results[0].weights.is_none(),
        "Weights should be None when include_weights=false"
    );
}
