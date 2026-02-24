mod common;
use common::test_source_ref;

use hauski_indexd::{ChunkPayload, IndexState, SearchRequest, UpsertRequest};
use serde_json::json;
use std::sync::Arc;

/// Integration test with a small fixture corpus (20+ events)
#[tokio::test]
async fn test_fixture_corpus_indexing_and_search() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Fixture 1-5: Rust programming topics
    for i in 1..=5 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("rust-{}", i),
                namespace: "code".into(),
                chunks: vec![ChunkPayload { text_lower: None,
                    chunk_id: Some(format!("rust-{}#0", i)),
                    text: Some(format!(
                        "Rust programming topic {}: memory safety and ownership",
                        i
                    )),
                    embedding: Vec::new(),
                    meta: json!({"topic": "rust", "id": i}),
                }],
                meta: json!({"language": "rust"}),
                source_ref: Some(test_source_ref("docs", format!("rust-{}.md", i))),
            })
            .await
            .expect("upsert should succeed");
    }

    // Fixture 6-10: Python scripting topics
    for i in 6..=10 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("python-{}", i),
                namespace: "code".into(),
                chunks: vec![ChunkPayload { text_lower: None,
                    chunk_id: Some(format!("python-{}#0", i)),
                    text: Some(format!("Python scripting tutorial {}: dynamic typing", i)),
                    embedding: Vec::new(),
                    meta: json!({"topic": "python", "id": i}),
                }],
                meta: json!({"language": "python"}),
                source_ref: Some(test_source_ref("docs", format!("python-{}.md", i))),
            })
            .await
            .expect("upsert should succeed");
    }

    // Fixture 11-15: System events (chronik namespace)
    for i in 11..=15 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("event-{}", i),
                namespace: "chronik".into(),
                chunks: vec![ChunkPayload { text_lower: None,
                    chunk_id: Some(format!("event-{}#0", i)),
                    text: Some(format!(
                        "System event {}: process started with high memory usage",
                        i
                    )),
                    embedding: Vec::new(),
                    meta: json!({"event_type": "process_start", "id": i}),
                }],
                meta: json!({"severity": "info"}),
                source_ref: Some(test_source_ref(
                    "chronik",
                    format!("/var/log/events/{}.log", i),
                )),
            })
            .await
            .expect("upsert should succeed");
    }

    // Fixture 16-20: Documentation snippets
    for i in 16..=20 {
        state
            .upsert(UpsertRequest {
                doc_id: format!("doc-{}", i),
                namespace: "docs".into(),
                chunks: vec![ChunkPayload { text_lower: None,
                    chunk_id: Some(format!("doc-{}#0", i)),
                    text: Some(format!("Documentation page {}: getting started guide", i)),
                    embedding: Vec::new(),
                    meta: json!({"section": "getting-started", "id": i}),
                }],
                meta: json!({"category": "tutorial"}),
                source_ref: Some(test_source_ref("docs", format!("page-{}.md", i))),
            })
            .await
            .expect("upsert should succeed");
    }

    // Test 1: Search for Rust in code namespace
    let rust_results = state
        .search(&SearchRequest {
            query: "rust".into(),
            k: Some(10),
            namespace: Some("code".into()),
            exclude_flags: None,
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
            emit_decision_snapshot: false,
        })
        .await;

    assert!(
        rust_results.len() >= 5,
        "Expected at least 5 Rust results, got {}",
        rust_results.len()
    );
    assert!(rust_results
        .iter()
        .all(|m| m.namespace == "code" && m.text.to_lowercase().contains("rust")));

    // Test 2: Search for events in chronik namespace
    let event_results = state
        .search(&SearchRequest {
            query: "process".into(),
            k: Some(10),
            namespace: Some("chronik".into()),
            exclude_flags: None,
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
            emit_decision_snapshot: false,
        })
        .await;

    assert!(
        event_results.len() >= 5,
        "Expected at least 5 event results, got {}",
        event_results.len()
    );
    assert!(event_results
        .iter()
        .all(|m| m.namespace == "chronik" && m.text.to_lowercase().contains("process")));

    // Test 3: Stats should show correct counts
    let stats = state.stats().await;
    assert_eq!(stats.total_documents, 20);
    assert_eq!(stats.total_chunks, 20);
    assert_eq!(stats.namespaces.len(), 3);
    assert_eq!(stats.namespaces.get("code"), Some(&10));
    assert_eq!(stats.namespaces.get("chronik"), Some(&5));
    assert_eq!(stats.namespaces.get("docs"), Some(&5));

    // Test 4: Related documents should work
    let related = state
        .related("rust-1".into(), Some(5), Some("code".into()))
        .await;

    // Should find other Rust documents as related (they share "rust" and "memory" words)
    assert!(
        !related.is_empty(),
        "Expected related documents, got empty list"
    );
    assert!(related.iter().any(|m| m.doc_id.starts_with("rust-")));
}

#[tokio::test]
async fn test_namespace_isolation() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    // Same text in different namespaces
    state
        .upsert(UpsertRequest {
            doc_id: "shared-doc".into(),
            namespace: "ns1".into(),
            chunks: vec![ChunkPayload { text_lower: None,
                chunk_id: Some("shared-doc#ns1".into()),
                text: Some("Shared content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "test-doc")),
        })
        .await
        .expect("upsert should succeed");

    state
        .upsert(UpsertRequest {
            doc_id: "shared-doc".into(),
            namespace: "ns2".into(),
            chunks: vec![ChunkPayload { text_lower: None,
                chunk_id: Some("shared-doc#ns2".into()),
                text: Some("Shared content".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "test-doc")),
        })
        .await
        .expect("upsert should succeed");

    // Search in ns1 should only return ns1 results
    let ns1_results = state
        .search(&SearchRequest {
            query: "shared".into(),
            k: Some(10),
            namespace: Some("ns1".into()),
            exclude_flags: None,
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
            emit_decision_snapshot: false,
        })
        .await;

    assert_eq!(ns1_results.len(), 1);
    assert_eq!(ns1_results[0].namespace, "ns1");

    // Search in ns2 should only return ns2 results
    let ns2_results = state
        .search(&SearchRequest {
            query: "shared".into(),
            k: Some(10),
            namespace: Some("ns2".into()),
            exclude_flags: None,
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
            emit_decision_snapshot: false,
        })
        .await;

    assert_eq!(ns2_results.len(), 1);
    assert_eq!(ns2_results[0].namespace, "ns2");
}

#[tokio::test]
async fn test_source_ref_and_ingested_at_populated() {
    let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

    state
        .upsert(UpsertRequest {
            doc_id: "doc-with-ref".into(),
            namespace: "default".into(),
            chunks: vec![ChunkPayload { text_lower: None,
                chunk_id: Some("doc-with-ref#0".into()),
                text: Some("Content with source".into()),
                embedding: Vec::new(),
                meta: json!({}),
            }],
            meta: json!({}),
            source_ref: Some(test_source_ref("chronik", "event-2024-01-01")),
        })
        .await
        .expect("upsert should succeed");

    let results = state
        .search(&SearchRequest {
            query: "content".into(),
            k: Some(1),
            namespace: None,
            exclude_flags: None,
            min_trust_level: None,
            exclude_origins: None,
            context_profile: None,
            include_weights: false,
            emit_decision_snapshot: false,
        })
        .await;

    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].source_ref,
        Some(test_source_ref("chronik", "event-2024-01-01"))
    );
    assert!(!results[0].ingested_at.is_empty());
    // Verify it's a valid RFC3339 timestamp
    assert!(chrono::DateTime::parse_from_rfc3339(&results[0].ingested_at).is_ok());
}
