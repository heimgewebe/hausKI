//! Test helpers and fixtures for indexd tests

use hauski_indexd::{SourceRef, TrustLevel};

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
