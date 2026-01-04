//! Test helpers and fixtures for indexd tests

use hauski_indexd::{SearchRequest, SourceRef, TrustLevel};

/// Helper to create test source refs with proper trust levels
pub fn test_source_ref(origin: &str, id: impl Into<String>) -> SourceRef {
    SourceRef {
        origin: origin.to_string(),
        id: id.into(),
        offset: None,
        trust_level: TrustLevel::default_for_origin(origin),
        injected_by: None,
    }
}

/// Helper to create a basic search request for testing (disables security filtering)
pub fn test_search_request(
    query: impl Into<String>,
    k: Option<usize>,
    namespace: Option<String>,
) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        k,
        namespace,
        exclude_flags: Some(vec![]), // Empty = no filtering for tests
        min_trust_level: None,
        exclude_origins: None,
    }
}
