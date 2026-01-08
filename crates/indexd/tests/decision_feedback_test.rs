mod common;
use common::test_source_ref;

use hauski_indexd::{
    ChunkPayload, DecisionOutcome, IndexState, OutcomeSignal, OutcomeSource, SearchRequest,
    UpsertRequest,
};
use serde_json::json;
use std::sync::Arc;

/// Test that decision snapshots are emitted when include_weights is true
#[tokio::test]
async fn test_decision_snapshot_emission() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Insert test documents
    state
        .upsert(UpsertRequest {
            doc_id: "doc-1".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-1#0".into()),
                text: Some("Rust programming language".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "doc-1")),
        })
        .await
        .expect("upsert should succeed");

    state
        .upsert(UpsertRequest {
            doc_id: "doc-2".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-2#0".into()),
                text: Some("Rust memory safety".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("osctx", "doc-2")),
        })
        .await
        .expect("upsert should succeed");

    // Search with include_weights=true to trigger snapshot emission
    let results = state
        .search(&SearchRequest {
            query: "Rust".into(),
            k: Some(10),
            namespace: Some("default".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: true, // This triggers snapshot emission
        })
        .await;

    assert_eq!(results.len(), 2);

    // Verify that snapshots were created
    let snapshots = state.list_decision_snapshots().await;
    assert_eq!(
        snapshots.len(),
        1,
        "One snapshot should have been emitted"
    );

    // Verify snapshot structure
    let snapshot = &snapshots[0];
    assert_eq!(snapshot.intent, "Rust");
    assert_eq!(snapshot.namespace, "default");
    assert_eq!(snapshot.candidates.len(), 2);
    assert!(snapshot.selected_id.is_some());
    assert_eq!(
        snapshot.selected_id.as_ref().unwrap(),
        &results[0].doc_id
    );

    // Verify candidate structure
    let candidate = &snapshot.candidates[0];
    assert!(candidate.similarity > 0.0);
    assert_eq!(candidate.weights.trust, 1.0); // High trust from chronik
    assert_eq!(candidate.weights.context, 1.0); // Default context
}

/// Test that decision snapshots are NOT emitted when include_weights is false
#[tokio::test]
async fn test_decision_snapshot_not_emitted_without_weights() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    state
        .upsert(UpsertRequest {
            doc_id: "doc-1".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-1#0".into()),
                text: Some("Test content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "doc-1")),
        })
        .await
        .expect("upsert should succeed");

    // Search without include_weights
    let results = state
        .search(&SearchRequest {
            query: "Test".into(),
            k: Some(10),
            namespace: Some("default".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false, // No snapshot should be emitted
        })
        .await;

    assert_eq!(results.len(), 1);

    // Verify no snapshots were created
    let snapshots = state.list_decision_snapshots().await;
    assert_eq!(
        snapshots.len(),
        0,
        "No snapshots should be emitted without include_weights"
    );
}

/// Test recording and retrieving decision outcomes
#[tokio::test]
async fn test_decision_outcome_recording() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Insert and search to create a snapshot
    state
        .upsert(UpsertRequest {
            doc_id: "doc-1".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-1#0".into()),
                text: Some("Test content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "doc-1")),
        })
        .await
        .expect("upsert should succeed");

    state
        .search(&SearchRequest {
            query: "Test".into(),
            k: Some(10),
            namespace: Some("default".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: true,
        })
        .await;

    // Get the decision ID
    let snapshots = state.list_decision_snapshots().await;
    assert_eq!(snapshots.len(), 1);
    let decision_id = snapshots[0].decision_id.clone();

    // Record an outcome
    let outcome = DecisionOutcome {
        decision_id: decision_id.clone(),
        outcome: OutcomeSignal::Success,
        signal_source: OutcomeSource::User,
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        notes: Some("User confirmed this was helpful".to_string()),
    };

    let result = state.record_outcome(outcome.clone()).await;
    assert!(result.is_ok(), "Recording outcome should succeed");

    // Retrieve the outcome
    let retrieved = state.get_decision_outcome(&decision_id).await;
    assert!(retrieved.is_some());

    let retrieved = retrieved.unwrap();
    assert_eq!(retrieved.decision_id, decision_id);
    assert_eq!(
        format!("{:?}", retrieved.outcome),
        format!("{:?}", OutcomeSignal::Success)
    );
    assert_eq!(
        format!("{:?}", retrieved.signal_source),
        format!("{:?}", OutcomeSource::User)
    );
    assert_eq!(
        retrieved.notes,
        Some("User confirmed this was helpful".to_string())
    );

    // List all outcomes
    let all_outcomes = state.list_decision_outcomes().await;
    assert_eq!(all_outcomes.len(), 1);
}

/// Test that recording outcome for non-existent decision fails
#[tokio::test]
async fn test_outcome_recording_fails_for_missing_decision() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    let outcome = DecisionOutcome {
        decision_id: "non-existent-decision-id".to_string(),
        outcome: OutcomeSignal::Success,
        signal_source: OutcomeSource::System,
        timestamp: "2024-01-01T00:00:00Z".to_string(),
        notes: None,
    };

    let result = state.record_outcome(outcome).await;
    assert!(
        result.is_err(),
        "Recording outcome for non-existent decision should fail"
    );

    let error = result.unwrap_err();
    assert_eq!(error.code, "decision_not_found");
}

/// Test that decision snapshots include policy hash for drift detection
#[tokio::test]
async fn test_decision_snapshot_includes_policy_hash() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    state
        .upsert(UpsertRequest {
            doc_id: "doc-1".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-1#0".into()),
                text: Some("Test content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "doc-1")),
        })
        .await
        .expect("upsert should succeed");

    state
        .search(&SearchRequest {
            query: "Test".into(),
            k: Some(10),
            namespace: Some("default".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: true,
        })
        .await;

    let snapshots = state.list_decision_snapshots().await;
    assert_eq!(snapshots.len(), 1);

    let snapshot = &snapshots[0];
    assert!(!snapshot.policy_hash.is_empty());
    assert_eq!(snapshot.policy_hash, state.policy_hash());
}
