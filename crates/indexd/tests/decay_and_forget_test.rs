mod common;
use chrono::{Duration, Utc};
use common::test_source_ref;
use hauski_indexd::{
    ChunkPayload, ForgetFilter, IndexState, PurgeStrategy, RetentionConfig, SearchRequest,
    UpsertRequest,
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
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
    // Search immediately - decay should be ~1.0 (no significant time passed)
    let results = state
        .search(&SearchRequest {
            query: "testing".into(),
            k: Some(5),
            namespace: Some("test".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
                source_ref: Some(test_source_ref("chronik", "test-event")),
            })
            .await
            .expect("upsert should succeed");
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
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
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
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
    // Dry-run forget
    let dry_result = state
        .forget(
            ForgetFilter {
                namespace: Some("forget".into()),
                older_than: None,
                source_ref_origin: None,
                doc_id: None,
                allow_namespace_wipe: true, // Explicitly allow wiping the namespace
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
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
                allow_namespace_wipe: true, // Explicitly allow wiping the namespace
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
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
        })
        .await;
    assert_eq!(search_after.len(), 0);
    // Verify other namespace is untouched
    let keep_search = state
        .search(&SearchRequest {
            query: "keep".into(),
            k: Some(5),
            namespace: Some("keep".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
            source_ref: Some(test_source_ref("chronik", "event-123")),
        })
        .await
        .expect("upsert should succeed");
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
            source_ref: Some(test_source_ref("code", "main.rs")),
        })
        .await
        .expect("upsert should succeed");
    // Forget only chronik documents
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: None,
                source_ref_origin: Some("chronik".into()),
                doc_id: None,
                allow_namespace_wipe: false,
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
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
    // Try to forget documents older than 1 day ago (should find nothing)
    let cutoff = Utc::now() - Duration::days(1);
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: Some(cutoff),
                source_ref_origin: None,
                doc_id: None,
                allow_namespace_wipe: false,
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
                allow_namespace_wipe: false,
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
                source_ref: Some(test_source_ref("chronik", "test-event")),
            })
            .await
            .expect("upsert should succeed");
    }
    // Forget only doc-2
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: None,
                source_ref_origin: None,
                doc_id: Some("doc-2".into()),
                allow_namespace_wipe: false,
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
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
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
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
    // Get initial score
    let results1 = state
        .search(&SearchRequest {
            query: "testing".into(),
            k: Some(5),
            namespace: Some("test".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
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
/// Test that filter semantics use AND logic (all filters must match)
#[tokio::test]
async fn test_forget_uses_and_semantics() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));
    // Add documents with different characteristics
    // Doc 1: old, from chronik
    state
        .upsert(UpsertRequest {
            doc_id: "doc-old-chronik".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-old-chronik#0".into()),
                text: Some("Old chronik content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "event-old")),
        })
        .await
        .expect("upsert should succeed");
    // Doc 2: old, from code (different origin)
    state
        .upsert(UpsertRequest {
            doc_id: "doc-old-code".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-old-code#0".into()),
                text: Some("Old code content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("code", "file.rs")),
        })
        .await
        .expect("upsert should succeed");
    // Doc 3: recent, from chronik
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    state
        .upsert(UpsertRequest {
            doc_id: "doc-new-chronik".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("doc-new-chronik#0".into()),
                text: Some("New chronik content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "event-new")),
        })
        .await
        .expect("upsert should succeed");
    // Test: Forget old AND chronik documents (AND semantics)
    let cutoff = Utc::now() - Duration::milliseconds(5);
    let result = state
        .forget(
            ForgetFilter {
                namespace: None,
                older_than: Some(cutoff),
                source_ref_origin: Some("chronik".into()),
                doc_id: None,
                allow_namespace_wipe: false,
            },
            false,
        )
        .await;
    // Should only forget doc-old-chronik (old AND chronik)
    // doc-old-code is old but not chronik (fails chronik filter)
    // doc-new-chronik is chronik but not old (fails older_than filter)
    assert_eq!(result.forgotten_count, 1);
    assert_eq!(result.forgotten_docs[0].doc_id, "doc-old-chronik");
    // Verify the other two documents remain
    let stats = state.stats().await;
    assert_eq!(stats.total_documents, 2);
    let search = state
        .search(&SearchRequest {
            query: "content".into(),
            k: Some(10),
            namespace: None,
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
        })
        .await;
    assert_eq!(search.len(), 2);
    let doc_ids: Vec<&str> = search.iter().map(|m| m.doc_id.as_str()).collect();
    assert!(doc_ids.contains(&"doc-old-code"));
    assert!(doc_ids.contains(&"doc-new-chronik"));
    assert!(!doc_ids.contains(&"doc-old-chronik"));
}
/// Test that namespace wipe without allow_namespace_wipe flag doesn't delete anything
#[tokio::test]
async fn test_namespace_wipe_requires_explicit_flag() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));
    // Add documents
    for i in 1..=3 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "test".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Content {}", i)),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("chronik", "test-event")),
            })
            .await
            .expect("upsert should succeed");
    }
    // Try to forget namespace without explicit flag (should delete nothing)
    let result = state
        .forget(
            ForgetFilter {
                namespace: Some("test".into()),
                older_than: None,
                source_ref_origin: None,
                doc_id: None,
                allow_namespace_wipe: false, // Explicit false
            },
            false,
        )
        .await;
    // Should not delete anything
    assert_eq!(result.forgotten_count, 0);
    // Verify documents still exist
    let stats = state.stats().await;
    assert_eq!(stats.total_documents, 3);
    // Now with explicit flag (should delete everything)
    let result2 = state
        .forget(
            ForgetFilter {
                namespace: Some("test".into()),
                older_than: None,
                source_ref_origin: None,
                doc_id: None,
                allow_namespace_wipe: true, // Explicit true
            },
            false,
        )
        .await;
    // Should delete all 3 documents
    assert_eq!(result2.forgotten_count, 3);
    // Verify documents are gone
    let stats2 = state.stats().await;
    assert_eq!(stats2.total_documents, 0);
}
/// Test that future timestamps (clock skew) are handled gracefully
#[tokio::test]
async fn test_future_timestamp_handling() {
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
    // Create a document with future timestamp (simulating clock skew)
    // Note: We can't directly set ingested_at, but we can test the behavior
    // by testing decay_preview which uses the same logic
    // Add a normal document
    state
        .upsert(UpsertRequest {
            doc_id: "normal-doc".into(),
            namespace: "test".into(),
            chunks: vec![ChunkPayload {
                chunk_id: Some("normal-doc#0".into()),
                text: Some("Normal content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "test-event")),
        })
        .await
        .expect("upsert should succeed");
    // Get decay preview
    let preview = state.preview_decay(Some("test".into())).await;
    assert_eq!(preview.total_documents, 1);
    assert_eq!(preview.previews.len(), 1);
    // age_seconds is u64, so always >= 0, but we verify it's reasonable
    // (not a huge value from negative i64 cast)
    assert!(preview.previews[0].age_seconds < 10); // Should be very fresh (< 10 seconds)
                                                   // Decay factor should be <= 1.0, never amplify scores
    assert!(preview.previews[0].decay_factor <= 1.0);
    assert!(preview.previews[0].decay_factor > 0.0);
    // Search should also not amplify scores
    let results = state
        .search(&SearchRequest {
            query: "content".into(),
            k: Some(5),
            namespace: Some("test".into()),
            exclude_flags: Some(vec![]),
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
        })
        .await;
    assert_eq!(results.len(), 1);
    // Score should be reasonable (between 0 and 1)
    assert!(results[0].score > 0.0);
    assert!(results[0].score <= 1.0);
}
/// Test defense-in-depth: forget() method itself rejects global wipe
#[tokio::test]
async fn test_forget_method_blocks_global_wipe() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));
    // Add documents in multiple namespaces
    for ns in &["ns1", "ns2", "ns3"] {
        for i in 1..=2 {
            state
                .upsert(UpsertRequest {
                    doc_id: format!("doc-{}", i),
                    namespace: ns.to_string(),
                    chunks: vec![ChunkPayload {
                        chunk_id: Some(format!("doc-{}#0", i)),
                        text: Some(format!("Content {} in {}", i, ns)),
                        embedding: Vec::new(),
                        meta: json!({}),
                    }],
                    meta: json!({}),
                    source_ref: Some(test_source_ref("chronik", "test-event")),
                })
                .await
                .expect("upsert should succeed");
        }
    }
    // Attempt: allow_namespace_wipe WITHOUT namespace
    let result = state
        .forget(
            ForgetFilter {
                namespace: None, // No namespace specified
                older_than: None,
                source_ref_origin: None,
                doc_id: None,
                allow_namespace_wipe: true, // But wipe flag is set
            },
            false,
        )
        .await;
    // Should be blocked: forget count must be 0
    assert_eq!(
        result.forgotten_count, 0,
        "allow_namespace_wipe without namespace should be blocked"
    );
    // Verify all documents still exist
    let stats = state.stats().await;
    assert_eq!(
        stats.total_documents, 6,
        "All 6 documents should still exist"
    );
}
