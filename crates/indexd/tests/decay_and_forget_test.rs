use chrono::{Duration, Utc};
use hauski_indexd::{
    ChunkPayload, ForgetFilter, IndexState, PurgeStrategy, RetentionConfig, SearchRequest,
    SourceRef, UpsertRequest,
};
use serde_json::json;
use std::sync::Arc;

/// Test that time-decay reduces scores over time
#[tokio::test]
async fn test_time_decay_reduces_scores() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Configure decay for namespace with 1-day half-life
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(86400), // 1 day
                max_items: None,
                max_age_seconds: None,
                purge_strategy: None,
            },
        )
        .await;

    // Create a document with known age by backdating ingestion
    // Note: In real implementation, we'd need to support custom ingested_at
    // For now, we test with freshly created documents and verify decay calculation

    state
        .upsert(UpsertRequest {
            doc_id: "recent-doc".into(),
            namespace: "test".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("recent-doc#0".into()),
                text: Some("Recent content about testing".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: None,
        })
        .await;

    // Search immediately - decay should be ~1.0 (no significant time passed)
    let results = state
        .search(&SearchRequest {
            query: "testing".into(),
            k: Some(5),
            namespace: Some("test".into()),
        })
        .await;

    assert_eq!(results.len(), 1);
    // Score should be close to base score (decay ~1.0 for fresh documents)
    assert!(results[0].score > 0.09); // Allowing for minor time passage
}

/// Test that decay preview shows correct decay factors
#[tokio::test]
async fn test_decay_preview() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Configure decay for namespace
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(3600), // 1 hour
                max_items: None,
                max_age_seconds: None,
                purge_strategy: None,
            },
        )
        .await;

    // Add test documents
    for i in 1..=3 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "test".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Test content {}", i)),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: None,
            })
            .await;
    }

    // Get decay preview
    let preview = state.preview_decay(Some("test".into())).await;

    assert_eq!(preview.namespace, "test");
    assert_eq!(preview.total_documents, 3);
    assert_eq!(preview.previews.len(), 3);

    // All documents should have decay_factor close to 1.0 (freshly created)
    for item in &preview.previews {
        assert!(item.decay_factor > 0.99);
        assert!(item.age_seconds < 5); // Should be very fresh
    }
}

/// Test intentional forget with namespace filter
#[tokio::test]
async fn test_forget_by_namespace() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Add documents to different namespaces
    state
        .upsert(UpsertRequest {
            doc_id: "keep-doc".into(),
            namespace: "keep".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("keep-doc#0".into()),
                text: Some("Keep this".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: None,
        })
        .await;

    state
        .upsert(UpsertRequest {
            doc_id: "forget-doc".into(),
            namespace: "forget".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("forget-doc#0".into()),
                text: Some("Forget this".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: None,
        })
        .await;

    // Dry-run forget
    let dry_result = state
        .forget(
            ForgetFilter {
                namespace: Some("forget".into()),
                older_than: None,
                source_ref_origin: None,
                doc_id: None,
            },
            true, // dry_run
        )
        .await;

    assert_eq!(dry_result.forgotten_count, 1);
    assert!(dry_result.dry_run);
    assert_eq!(dry_result.forgotten_docs.len(), 1);
    assert_eq!(dry_result.forgotten_docs[0].namespace, "forget");

    // Verify document still exists (dry-run)
    let search_after_dry = state
        .search(&SearchRequest {
            query: "forget".into(),
            k: Some(5),
            namespace: Some("forget".into()),
        })
        .await;
    assert_eq!(search_after_dry.len(), 1);

    // Actual forget
    let result = state
        .forget(
            ForgetFilter {
                namespace: Some("forget".into()),
                older_than: None,
                source_ref_origin: None,
                doc_id: None,
            },
            false, // not dry_run
        )
        .await;

    assert_eq!(result.forgotten_count, 1);
    assert!(!result.dry_run);

    // Verify document is gone
    let search_after = state
        .search(&SearchRequest {
            query: "forget".into(),
            k: Some(5),
            namespace: Some("forget".into()),
        })
        .await;
    assert_eq!(search_after.len(), 0);

    // Verify other namespace is untouched
    let keep_search = state
        .search(&SearchRequest {
            query: "keep".into(),
            k: Some(5),
            namespace: Some("keep".into()),
        })
        .await;
    assert_eq!(keep_search.len(), 1);
}

/// Test forget with source_ref filter
#[tokio::test]
async fn test_forget_by_source_ref_origin() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Add documents with different source origins
    state
        .upsert(UpsertRequest {
            doc_id: "chronik-doc".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("chronik-doc#0".into()),
                text: Some("System event log".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(SourceRef {
                origin: "chronik".into(),
                id: "event-123".into(),
                offset: None,
            }),
        })
        .await;

    state
        .upsert(UpsertRequest {
            doc_id: "code-doc".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("code-doc#0".into()),
                text: Some("Source code snippet".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(SourceRef {
                origin: "code".into(),
                id: "main.rs".into(),
                offset: Some("line:42".into()),
            }),
        })
        .await;

    // Forget only chronik documents
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: None,
                source_ref_origin: Some("chronik".into()),
                doc_id: None,
            },
            false,
        )
        .await;

    assert_eq!(result.forgotten_count, 1);
    assert_eq!(result.forgotten_docs[0].doc_id, "chronik-doc");

    // Verify code document remains
    let search_code = state
        .search(&SearchRequest {
            query: "source".into(),
            k: Some(5),
            namespace: None,
        })
        .await;
    assert_eq!(search_code.len(), 1);
    assert_eq!(search_code[0].doc_id, "code-doc");
}

/// Test forget with older_than filter
#[tokio::test]
async fn test_forget_older_than() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Add a document
    state
        .upsert(UpsertRequest {
            doc_id: "old-doc".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("old-doc#0".into()),
                text: Some("Old content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: None,
        })
        .await;

    // Try to forget documents older than 1 day ago (should find nothing)
    let cutoff = Utc::now() - Duration::days(1);
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: Some(cutoff),
                source_ref_origin: None,
                doc_id: None,
            },
            false,
        )
        .await;

    // Should not forget anything (document is fresh)
    assert_eq!(result.forgotten_count, 0);

    // Try to forget documents older than 1 second in the future (should find the doc)
    let future_cutoff = Utc::now() + Duration::seconds(1);
    let result2 = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: Some(future_cutoff),
                source_ref_origin: None,
                doc_id: None,
            },
            false,
        )
        .await;

    // Should forget the document
    assert_eq!(result2.forgotten_count, 1);
}

/// Test forget with specific doc_id
#[tokio::test]
async fn test_forget_by_doc_id() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Add multiple documents
    for i in 1..=3 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Content {}", i)),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: None,
            })
            .await;
    }

    // Forget only doc-2
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: None,
                source_ref_origin: None,
                doc_id: Some("doc-2".into()),
            },
            false,
        )
        .await;

    assert_eq!(result.forgotten_count, 1);
    assert_eq!(result.forgotten_docs[0].doc_id, "doc-2");

    // Verify stats
    let stats = state.stats().await;
    assert_eq!(stats.total_documents, 2);

    // Verify doc-1 and doc-3 remain
    let search = state
        .search(&SearchRequest {
            query: "content".into(),
            k: Some(10),
            namespace: None,
        })
        .await;

    assert_eq!(search.len(), 2);
    let doc_ids: Vec<&str> = search.iter().map(|m| m.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"doc-1"));
    assert!(doc_ids.contains(&"doc-3"));
    assert!(!doc_ids.contains(&"doc-2"));
}

/// Test retention config retrieval
#[tokio::test]
async fn test_get_retention_configs() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Set multiple retention configs
    state
        .set_retention_config(
            "chronik".into(),
            RetentionConfig {
                half_life_seconds: Some(2592000),
                max_items: Some(10000),
                max_age_seconds: Some(7776000),
                purge_strategy: Some(PurgeStrategy::Oldest),
            },
        )
        .await;

    state
        .set_retention_config(
            "code".into(),
            RetentionConfig {
                half_life_seconds: None,
                max_items: Some(50000),
                max_age_seconds: None,
                purge_strategy: Some(PurgeStrategy::LowestScore),
            },
        )
        .await;

    // Retrieve configs
    let configs = state.get_retention_configs().await;

    assert_eq!(configs.len(), 2);
    assert!(configs.contains_key("chronik"));
    assert!(configs.contains_key("code"));

    let chronik_config = configs.get("chronik").unwrap();
    assert_eq!(chronik_config.half_life_seconds, Some(2592000));
    assert_eq!(chronik_config.max_items, Some(10000));
    assert_eq!(chronik_config.purge_strategy, Some(PurgeStrategy::Oldest));

    let code_config = configs.get("code").unwrap();
    assert_eq!(code_config.half_life_seconds, None);
    assert_eq!(code_config.max_items, Some(50000));
    assert_eq!(code_config.purge_strategy, Some(PurgeStrategy::LowestScore));
}

/// Test that decay calculation is deterministic
#[tokio::test]
async fn test_decay_calculation_deterministic() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Configure decay
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(3600), // 1 hour
                max_items: None,
                max_age_seconds: None,
                purge_strategy: None,
            },
        )
        .await;

    // Add document
    state
        .upsert(UpsertRequest {
            doc_id: "test-doc".into(),
            namespace: "test".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("test-doc#0".into()),
                text: Some("Consistent decay test".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: None,
        })
        .await;

    // Get preview twice
    let preview1 = state.preview_decay(Some("test".into())).await;
    let preview2 = state.preview_decay(Some("test".into())).await;

    // Both should have same number of results
    assert_eq!(preview1.previews.len(), preview2.previews.len());

    // Decay factors should be very close (allowing for minimal time passage)
    for (p1, p2) in preview1.previews.iter().zip(preview2.previews.iter()) {
        let diff = (p1.decay_factor - p2.decay_factor).abs();
        assert!(diff < 0.001, "Decay factors should be consistent");
    }
}

/// Integration test: decay affects search ranking
#[tokio::test]
async fn test_decay_affects_search_ranking() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

    // Configure very aggressive decay for testing (1 second half-life)
    state
        .set_retention_config(
            "test".into(),
            RetentionConfig {
                half_life_seconds: Some(1), // 1 second half-life
                max_items: None,
                max_age_seconds: None,
                purge_strategy: None,
            },
        )
        .await;

    // Add document
    state
        .upsert(UpsertRequest {
            doc_id: "decay-doc".into(),
            namespace: "test".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("decay-doc#0".into()),
                text: Some("testing decay ranking".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: None,
        })
        .await;

    // Get initial score
    let results1 = state
        .search(&SearchRequest {
            query: "testing".into(),
            k: Some(5),
            namespace: Some("test".into()),
        })
        .await;

    assert_eq!(results1.len(), 1);
    let initial_score = results1[0].score;

    // Wait a bit for decay to take effect
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Get score after waiting
    let results2 = state
        .search(&SearchRequest {
            query: "testing".into(),
            k: Some(5),
            namespace: Some("test".into()),
        })
        .await;

    assert_eq!(results2.len(), 1);
    let decayed_score = results2[0].score;

    // Score should have decreased due to decay
    assert!(
        decayed_score < initial_score,
        "Decayed score ({}) should be less than initial score ({})",
        decayed_score,
        initial_score
    );

    // With 1-second half-life and 2 seconds elapsed, decay should be ~0.25
    // So score should be roughly 1/4 of original
    let expected_decay_factor = 0.25;
    let actual_decay_factor = decayed_score / initial_score;

    // Allow some tolerance for timing imprecision
    assert!(
        (actual_decay_factor - expected_decay_factor).abs() < 0.1,
        "Decay factor {} should be close to expected {}",
        actual_decay_factor,
        expected_decay_factor
    );
}
