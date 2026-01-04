use axum::{
    extract::{FromRef, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{borrow::Cow, cmp::Ordering, collections::HashMap, sync::Arc, time::Instant};
use tokio::sync::RwLock;

const DEFAULT_NAMESPACE: &str = "default";
const QUARANTINE_NAMESPACE: &str = "quarantine";
const MIN_WORD_LENGTH_FOR_SIMILARITY: usize = 3;
const WORD_MATCH_SCORE_INCREMENT: f32 = 0.1;

pub type MetricsRecorder = dyn Fn(Method, &'static str, StatusCode, Instant) + Send + Sync;

/// Error type for index operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexError {
    pub error: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl IndexError {
    pub fn missing_source_ref() -> Self {
        Self {
            error: "source_ref is required for all index entries".into(),
            code: "missing_source_ref".into(),
            details: Some(serde_json::json!({
                "hint": "Every document must have a SourceRef with origin, id, and trust_level for semantic provenance tracking"
            })),
        }
    }
}

/// Trust level for document sources - indicates how much to trust this content
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// Low trust - external sources, user input, tool output
    Low,
    /// Medium trust - OS context, application logs
    Medium,
    /// High trust - chronik events, verified internal sources
    High,
}

impl TrustLevel {
    /// Returns the default trust level for a given origin
    pub fn default_for_origin(origin: &str) -> Self {
        match origin {
            "chronik" => TrustLevel::High,
            "osctx" => TrustLevel::Medium,
            "user" | "external" | "tool" => TrustLevel::Low,
            _ => TrustLevel::Medium,
        }
    }
}

/// Content flags indicating potential security or quality issues
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContentFlag {
    /// Content contains possible prompt injection patterns
    PossiblePromptInjection,
    /// Content contains imperative language
    ImperativeLanguage,
    /// Content contains system claims or policy overrides
    SystemClaim,
    /// Content contains meta-prompt markers
    MetaPromptMarker,
}

impl std::fmt::Display for ContentFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContentFlag::PossiblePromptInjection => write!(f, "possible_prompt_injection"),
            ContentFlag::ImperativeLanguage => write!(f, "imperative_language"),
            ContentFlag::SystemClaim => write!(f, "system_claim"),
            ContentFlag::MetaPromptMarker => write!(f, "meta_prompt_marker"),
        }
    }
}

/// Detect potential prompt injection patterns in text
/// Returns a set of flags indicating issues found
fn detect_injection_patterns(text: &str) -> Vec<ContentFlag> {
    let mut flags = Vec::new();
    let text_lower = text.to_lowercase();

    // Imperative language patterns
    let imperative_patterns = [
        "du sollst",
        "du musst",
        "you must",
        "you should",
        "ignore previous",
        "disregard",
        "forget everything",
    ];
    
    for pattern in &imperative_patterns {
        if text_lower.contains(pattern) {
            flags.push(ContentFlag::ImperativeLanguage);
            break;
        }
    }

    // System claims and policy overrides
    let system_patterns = [
        "this system must",
        "system prompt",
        "policy override",
        "override policy",
        "system instruction",
        "admin mode",
        "bypass",
    ];
    
    for pattern in &system_patterns {
        if text_lower.contains(pattern) {
            flags.push(ContentFlag::SystemClaim);
            break;
        }
    }

    // Meta-prompt markers
    let meta_patterns = [
        "as an ai",
        "as a language model",
        "i am an ai",
        "i'm an ai",
        "assistant mode",
        "system role",
    ];
    
    for pattern in &meta_patterns {
        if text_lower.contains(pattern) {
            flags.push(ContentFlag::MetaPromptMarker);
            break;
        }
    }

    // If multiple flags, add overall prompt injection flag
    if flags.len() >= 2 {
        flags.push(ContentFlag::PossiblePromptInjection);
    }

    flags
}

/// Determine if a document should be quarantined based on flags and trust level
/// 
/// Quarantine policy:
/// - High trust: Never auto-quarantine (only flag for visibility)
/// - Medium trust: Quarantine only if PossiblePromptInjection flag is present
/// - Low trust: Quarantine if 2+ flags OR PossiblePromptInjection flag
fn should_quarantine(flags: &[ContentFlag], trust_level: TrustLevel) -> bool {
    match trust_level {
        TrustLevel::High => false, // High trust sources are never auto-quarantined
        TrustLevel::Medium => flags.contains(&ContentFlag::PossiblePromptInjection),
        TrustLevel::Low => {
            flags.len() >= 2 || flags.contains(&ContentFlag::PossiblePromptInjection)
        }
    }
}

/// Structured reference to document source for provenance tracking.
/// This replaces the previous Option<String> to provide clear semantics.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceRef {
    /// Origin namespace or system (e.g., "chronik", "osctx", "user", "tool", "external")
    pub origin: String,
    /// Unique identifier within the origin (e.g., event_id, file path, hash)
    pub id: String,
    /// Optional location within the source (e.g., "line:42", "byte:1337-2048")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<String>,
    /// Trust level - how much to trust this content
    pub trust_level: TrustLevel,
    /// Optional agent or tool that injected this content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injected_by: Option<String>,
}

/// Retention configuration for a namespace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// Time-decay half-life in seconds (None = no decay)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub half_life_seconds: Option<u64>,

    /// Maximum number of items in namespace (None = unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_items: Option<usize>,

    /// Maximum age of items in seconds (None = unlimited)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_seconds: Option<u64>,

    /// Purge strategy when limits are exceeded
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purge_strategy: Option<PurgeStrategy>,
}

/// Strategy for purging old items when retention limits are exceeded
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PurgeStrategy {
    /// Remove oldest items first (FIFO)
    Oldest,
    /// Remove items with lowest combined score (decay + relevance)
    LowestScore,
}

/// Reason for forgetting/deletion
///
/// This enum is intended for use in metrics and structured logging
/// to track why documents are being forgotten. Currently exported
/// for future integration with metrics recording (Phase 6).
///
/// Example future usage:
/// ```ignore
/// metrics.record_forgotten(namespace, ForgetReason::Manual, count);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgetReason {
    /// Time-to-live exceeded
    Ttl,
    /// Namespace retention policy triggered
    Retention,
    /// Manual/intentional deletion
    Manual,
}

impl std::fmt::Display for ForgetReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForgetReason::Ttl => write!(f, "ttl"),
            ForgetReason::Retention => write!(f, "retention"),
            ForgetReason::Manual => write!(f, "manual"),
        }
    }
}

/// Calculate decay factor based on age and half-life
/// Returns 1.0 if half_life is None (no decay)
fn calculate_decay_factor(age_seconds: i64, half_life_seconds: Option<u64>) -> f32 {
    match half_life_seconds {
        None => 1.0,
        Some(0) => 1.0, // Avoid division by zero
        Some(half_life) => {
            let exponent = age_seconds as f64 / half_life as f64;
            0.5_f64.powf(exponent) as f32
        }
    }
}

fn normalize_namespace(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        DEFAULT_NAMESPACE.to_string()
    } else {
        trimmed.to_string()
    }
}

fn resolve_namespace(namespace: Option<&str>) -> Cow<'_, str> {
    match namespace {
        Some(raw) => Cow::Owned(normalize_namespace(raw)),
        None => Cow::Borrowed(DEFAULT_NAMESPACE),
    }
}

#[derive(Clone)]
pub struct IndexState {
    inner: Arc<IndexInner>,
}

struct IndexInner {
    store: RwLock<HashMap<String, NamespaceStore>>,
    metrics: Arc<MetricsRecorder>,
    budget_ms: u64,
    retention_configs: RwLock<HashMap<String, RetentionConfig>>,
}

type NamespaceStore = HashMap<String, DocumentRecord>;

#[derive(Clone, Debug)]
struct DocumentRecord {
    doc_id: String,
    namespace: String,
    chunks: Vec<ChunkPayload>,
    meta: Value,
    /// Structured source reference for provenance tracking (mandatory for semantic security)
    source_ref: Option<SourceRef>,
    /// System-generated ingestion timestamp (always present, set at document creation)
    ingested_at: DateTime<Utc>,
    /// Content flags indicating potential security or quality issues
    flags: Vec<ContentFlag>,
}

impl IndexState {
    pub fn new(budget_ms: u64, metrics: Arc<MetricsRecorder>) -> Self {
        Self {
            inner: Arc::new(IndexInner {
                store: RwLock::new(HashMap::new()),
                metrics,
                budget_ms,
                retention_configs: RwLock::new(HashMap::new()),
            }),
        }
    }

    pub fn budget_ms(&self) -> u64 {
        self.inner.budget_ms
    }

    fn record(&self, method: Method, path: &'static str, status: StatusCode, started: Instant) {
        (self.inner.metrics)(method, path, status, started);
    }

    pub async fn upsert(&self, payload: UpsertRequest) -> Result<usize, IndexError> {
        let UpsertRequest {
            doc_id,
            namespace,
            chunks,
            meta,
            source_ref,
        } = payload;
        
        // Enforce source_ref requirement for semantic security
        let source_ref = source_ref.ok_or_else(IndexError::missing_source_ref)?;
        
        // Detect injection patterns in all chunk text
        let mut flags = Vec::new();
        for chunk in &chunks {
            if let Some(text) = &chunk.text {
                let chunk_flags = detect_injection_patterns(text);
                for flag in chunk_flags {
                    if !flags.contains(&flag) {
                        flags.push(flag);
                    }
                }
            }
        }
        
        // Trust-gated auto-quarantine
        let mut target_namespace = normalize_namespace(&namespace);
        if should_quarantine(&flags, source_ref.trust_level) {
            tracing::warn!(
                doc_id = %doc_id,
                flags = ?flags,
                trust_level = ?source_ref.trust_level,
                origin = %source_ref.origin,
                original_namespace = %target_namespace,
                "Auto-quarantining document based on trust level and injection flags"
            );
            target_namespace = QUARANTINE_NAMESPACE.to_string();
        }
        
        let mut store = self.inner.store.write().await;
        let namespace_store = store.entry(target_namespace.clone()).or_insert_with(HashMap::new);
        let ingested = chunks.len();
        
        // Log flag detection (even if not quarantined)
        if !flags.is_empty() {
            tracing::info!(
                doc_id = %doc_id,
                namespace = %target_namespace,
                flags = ?flags,
                trust_level = ?source_ref.trust_level,
                "Document flagged during upsert"
            );
        }
        
        namespace_store.insert(
            doc_id.clone(),
            DocumentRecord {
                doc_id,
                namespace: target_namespace.clone(),
                chunks,
                meta,
                source_ref: Some(source_ref),
                ingested_at: Utc::now(),
                flags,
            },
        );
        Ok(ingested)
    }

    pub async fn search(&self, request: &SearchRequest) -> Vec<SearchMatch> {
        let query = request.query.trim();
        if query.is_empty() {
            return Vec::new();
        }

        let store = self.inner.store.read().await;
        let retention_configs = self.inner.retention_configs.read().await;
        let namespace = resolve_namespace(request.namespace.as_deref());
        let Some(namespace_store) = store.get(namespace.as_ref()) else {
            return Vec::new();
        };
        let limit = request.k.unwrap_or(20).min(100);
        let query_lower = query.to_lowercase();
        let query_char_len = query_lower.chars().count();
        let query_byte_len = query_lower.len();
        let now = Utc::now();

        // Get retention config for namespace (if any)
        let retention_config = retention_configs.get(namespace.as_ref());
        
        // Prepare filter criteria (use typed enums, not strings)
        let exclude_flags_set = request.effective_exclude_flags();
        let min_trust = request.min_trust_level;
        let exclude_origins_set: Vec<String> = request.exclude_origins.clone().unwrap_or_default();

        let mut matches: Vec<SearchMatch> = Vec::new();
        let mut filtered_count = 0;
        
        for doc in namespace_store.values() {
            // Apply trust level filter
            if let Some(min_trust_level) = min_trust {
                if let Some(ref source_ref) = doc.source_ref {
                    if source_ref.trust_level < min_trust_level {
                        filtered_count += 1;
                        continue;
                    }
                }
            }
            
            // Apply origin filter
            if !exclude_origins_set.is_empty() {
                if let Some(ref source_ref) = doc.source_ref {
                    if exclude_origins_set.contains(&source_ref.origin) {
                        filtered_count += 1;
                        continue;
                    }
                }
            }
            
            // Apply flag filter (now using enum comparison)
            let has_excluded_flag = doc.flags.iter().any(|flag| {
                exclude_flags_set.contains(flag)
            });
            if has_excluded_flag {
                filtered_count += 1;
                continue;
            }
            
            for (idx, chunk) in doc.chunks.iter().enumerate() {
                let Some(text) = chunk.text.as_ref() else {
                    continue;
                };

                let Some(base_score) =
                    substring_match_score(text, &query_lower, query_byte_len, query_char_len)
                else {
                    continue;
                };

                // Apply time-decay if configured
                // Clamp age to 0 to handle future timestamps gracefully (clock skew)
                let age_seconds = (now - doc.ingested_at).num_seconds().max(0);
                let decay_factor = if let Some(config) = retention_config {
                    calculate_decay_factor(age_seconds, config.half_life_seconds)
                } else {
                    1.0
                };
                let final_score = base_score * decay_factor;

                matches.push(SearchMatch {
                    doc_id: doc.doc_id.clone(),
                    namespace: doc.namespace.clone(),
                    chunk_id: chunk
                        .chunk_id
                        .clone()
                        .unwrap_or_else(|| format!("{}#{idx}", doc.doc_id)),
                    score: final_score,
                    text: text.clone(),
                    meta: if chunk.meta.is_null() {
                        doc.meta.clone()
                    } else {
                        chunk.meta.clone()
                    },
                    source_ref: doc.source_ref.clone(),
                    ingested_at: doc.ingested_at.to_rfc3339(),
                    flags: doc.flags.clone(),
                });
            }
        }
        
        // Log filter statistics
        if filtered_count > 0 {
            tracing::debug!(
                namespace = %namespace,
                filtered_count = filtered_count,
                "Documents filtered during search due to security policies"
            );
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        if matches.len() > limit {
            matches.truncate(limit);
        }
        matches
    }

    pub async fn stats(&self) -> StatsResponse {
        let store = self.inner.store.read().await;
        let mut total_docs = 0;
        let mut total_chunks = 0;
        let mut namespace_counts = HashMap::new();

        for (namespace, namespace_store) in store.iter() {
            let doc_count = namespace_store.len();
            let chunk_count: usize = namespace_store.values().map(|doc| doc.chunks.len()).sum();

            total_docs += doc_count;
            total_chunks += chunk_count;
            namespace_counts.insert(namespace.clone(), doc_count);
        }

        StatsResponse {
            total_documents: total_docs,
            total_chunks,
            namespaces: namespace_counts,
            budget_ms: self.inner.budget_ms,
        }
    }

    pub async fn related(
        &self,
        doc_id: String,
        k: Option<usize>,
        namespace: Option<String>,
    ) -> Vec<SearchMatch> {
        let store = self.inner.store.read().await;
        let namespace = resolve_namespace(namespace.as_deref());
        let Some(namespace_store) = store.get(namespace.as_ref()) else {
            return Vec::new();
        };

        let Some(source_doc) = namespace_store.get(&doc_id) else {
            return Vec::new();
        };

        let limit = k.unwrap_or(20).min(100);
        let mut matches: Vec<SearchMatch> = Vec::new();

        // Pre-compute source text once (outside loops for performance)
        let source_text: Vec<String> = source_doc
            .chunks
            .iter()
            .filter_map(|c| c.text.as_ref().map(|t| t.to_lowercase()))
            .collect();

        // For now, use simple text-based similarity (compare all chunks with source)
        // In future: use embedding-based similarity
        for (other_doc_id, other_doc) in namespace_store.iter() {
            if other_doc_id == &doc_id {
                continue; // skip self
            }

            for (idx, chunk) in other_doc.chunks.iter().enumerate() {
                let Some(text) = chunk.text.as_ref() else {
                    continue;
                };

                // Simple heuristic: calculate overlap with source document text
                let text_lower = text.to_lowercase();
                let mut score = 0.0f32;
                for src_text in &source_text {
                    let words: Vec<&str> = src_text.split_whitespace().collect();
                    for word in words {
                        if word.len() > MIN_WORD_LENGTH_FOR_SIMILARITY && text_lower.contains(word)
                        {
                            score += WORD_MATCH_SCORE_INCREMENT;
                        }
                    }
                }

                if score > 0.0 {
                    matches.push(SearchMatch {
                        doc_id: other_doc.doc_id.clone(),
                        namespace: other_doc.namespace.clone(),
                        chunk_id: chunk
                            .chunk_id
                            .clone()
                            .unwrap_or_else(|| format!("{}#{idx}", other_doc.doc_id)),
                        score,
                        text: text.clone(),
                        meta: if chunk.meta.is_null() {
                            other_doc.meta.clone()
                        } else {
                            chunk.meta.clone()
                        },
                        source_ref: other_doc.source_ref.clone(),
                        ingested_at: other_doc.ingested_at.to_rfc3339(),
                        flags: other_doc.flags.clone(),
                    });
                }
            }
        }

        matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
        if matches.len() > limit {
            matches.truncate(limit);
        }
        matches
    }

    /// Set retention configuration for a namespace
    pub async fn set_retention_config(&self, namespace: String, config: RetentionConfig) {
        let namespace = normalize_namespace(&namespace);
        let mut configs = self.inner.retention_configs.write().await;
        configs.insert(namespace, config);
    }

    /// Get all retention configurations
    pub async fn get_retention_configs(&self) -> HashMap<String, RetentionConfig> {
        let configs = self.inner.retention_configs.read().await;
        configs.clone()
    }

    /// Forget (delete) documents matching the given filter
    /// Returns the number of documents forgotten
    ///
    /// Filter semantics: Uses AND logic - ALL specified filters must match for a document to be forgotten.
    ///
    /// Safety guarantees:
    /// - At least one content filter (older_than, source_ref_origin, doc_id) must be specified,
    ///   OR namespace must be set with allow_namespace_wipe=true
    /// - Without content filters and allow_namespace_wipe=false, no documents are forgotten
    /// - allow_namespace_wipe requires namespace to be specified (prevents cross-namespace deletion)
    /// - This prevents accidental global or namespace-wide deletion
    pub async fn forget(&self, filter: ForgetFilter, dry_run: bool) -> ForgetResult {
        let mut store = self.inner.store.write().await;
        let mut forgotten_count = 0;
        let mut forgotten_docs = Vec::new();

        // Critical safety check: allow_namespace_wipe without namespace is forbidden
        // This prevents global deletion across all namespaces
        if filter.allow_namespace_wipe && filter.namespace.is_none() {
            tracing::warn!(
                "Blocked forget operation: allow_namespace_wipe=true without namespace specified"
            );
            return ForgetResult {
                forgotten_count: 0,
                forgotten_docs: Vec::new(),
                dry_run,
            };
        }

        // Determine which namespaces to process
        let namespaces_to_check: Vec<String> = if let Some(ref filter_ns) = filter.namespace {
            // Specific namespace requested
            if store.contains_key(filter_ns) {
                vec![filter_ns.clone()]
            } else {
                vec![]
            }
        } else {
            // No namespace filter - iterate all namespaces
            store.keys().cloned().collect()
        };

        // Check if we have at least one content filter
        let has_content_filters = filter.older_than.is_some()
            || filter.source_ref_origin.is_some()
            || filter.doc_id.is_some();

        for namespace_name in namespaces_to_check {
            let namespace_store = match store.get_mut(&namespace_name) {
                Some(ns) => ns,
                None => continue,
            };

            let mut to_remove = Vec::new();

            for (doc_id, doc) in namespace_store.iter() {
                // Start with true, then apply AND logic for all filters
                let mut should_forget = true;

                // If no content filters and namespace wipe not explicitly allowed, skip everything
                if !has_content_filters && !filter.allow_namespace_wipe {
                    should_forget = false;
                }

                // Apply older_than filter (if specified)
                if let Some(older_than) = filter.older_than {
                    should_forget = should_forget && (doc.ingested_at < older_than);
                }

                // Apply source_ref filter (if specified)
                if let Some(ref filter_origin) = filter.source_ref_origin {
                    let matches_origin = doc
                        .source_ref
                        .as_ref()
                        .map(|sr| &sr.origin == filter_origin)
                        .unwrap_or(false);
                    should_forget = should_forget && matches_origin;
                }

                // Apply doc_id filter (if specified)
                if let Some(ref filter_doc_id) = filter.doc_id {
                    should_forget = should_forget && (doc_id == filter_doc_id);
                }

                if should_forget {
                    to_remove.push(doc_id.clone());
                    forgotten_docs.push(ForgottenDocument {
                        doc_id: doc_id.clone(),
                        namespace: namespace_name.clone(),
                        ingested_at: doc.ingested_at.to_rfc3339(),
                    });
                }
            }

            if !dry_run {
                for doc_id in &to_remove {
                    namespace_store.remove(doc_id);
                }
            }

            forgotten_count += to_remove.len();
        }

        ForgetResult {
            forgotten_count,
            dry_run,
            forgotten_docs,
        }
    }

    /// Preview decay effect without modifying scores
    pub async fn preview_decay(&self, namespace: Option<String>) -> DecayPreview {
        let store = self.inner.store.read().await;
        let retention_configs = self.inner.retention_configs.read().await;
        let namespace = resolve_namespace(namespace.as_deref());

        let mut previews = Vec::new();
        let now = Utc::now();

        if let Some(namespace_store) = store.get(namespace.as_ref()) {
            let retention_config = retention_configs.get(namespace.as_ref());

            for doc in namespace_store.values() {
                // Clamp age to 0 to handle future timestamps gracefully (clock skew)
                let age_seconds = (now - doc.ingested_at).num_seconds().max(0);
                let decay_factor = if let Some(config) = retention_config {
                    calculate_decay_factor(age_seconds, config.half_life_seconds)
                } else {
                    1.0
                };

                previews.push(DecayPreviewItem {
                    doc_id: doc.doc_id.clone(),
                    namespace: doc.namespace.clone(),
                    ingested_at: doc.ingested_at.to_rfc3339(),
                    age_seconds: age_seconds as u64,
                    decay_factor,
                });
            }
        }

        previews.sort_by(|a, b| {
            a.decay_factor
                .partial_cmp(&b.decay_factor)
                .unwrap_or(Ordering::Equal)
        });

        DecayPreview {
            namespace: namespace.to_string(),
            total_documents: previews.len(),
            previews,
        }
    }
}

fn substring_match_score(
    text: &str,
    query_lower: &str,
    query_byte_len: usize,
    query_char_len: usize,
) -> Option<f32> {
    if query_byte_len == 0 || query_char_len == 0 {
        return None;
    }

    let text_lower = text.to_lowercase();
    let mut count = 0;
    let mut remaining = text_lower.as_str();

    while let Some(pos) = remaining.find(query_lower) {
        count += 1;
        let advance = pos + query_byte_len;
        if advance >= remaining.len() {
            remaining = "";
        } else {
            remaining = &remaining[advance..];
        }
    }

    if count == 0 {
        return None;
    }

    let text_char_len = text_lower.chars().count().max(1);
    let matched_chars = count * query_char_len;
    Some((matched_chars as f32 / text_char_len as f32).min(1.0))
}

pub fn router<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
    IndexState: FromRef<S>,
{
    // Note: This router is nested under /index in core (see core/src/lib.rs),
    // so routes like /stats become /index/stats when mounted.
    // Metrics are recorded with full paths (/index/stats, etc.) for consistency.
    Router::<S>::new()
        .route("/upsert", post(upsert_handler))
        .route("/search", post(search_handler))
        .route("/stats", axum::routing::get(stats_handler))
        .route("/related", post(related_handler))
        .route("/forget", post(forget_handler))
        .route("/retention", axum::routing::get(retention_handler))
        .route("/decay/preview", post(decay_preview_handler))
}

async fn upsert_handler(
    State(state): State<IndexState>,
    Json(payload): Json<UpsertRequest>,
) -> Response {
    let started = Instant::now();
    
    match state.upsert(payload).await {
        Ok(ingested) => {
            state.record(Method::POST, "/index/upsert", StatusCode::OK, started);
            (
                StatusCode::OK,
                Json(UpsertResponse {
                    status: "queued".into(),
                    ingested,
                }),
            )
                .into_response()
        }
        Err(error) => {
            state.record(Method::POST, "/index/upsert", StatusCode::UNPROCESSABLE_ENTITY, started);
            (StatusCode::UNPROCESSABLE_ENTITY, Json(error)).into_response()
        }
    }
}

async fn search_handler(
    State(state): State<IndexState>,
    Json(payload): Json<SearchRequest>,
) -> Response {
    let started = Instant::now();
    let matches = state.search(&payload).await;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    state.record(Method::POST, "/index/search", StatusCode::OK, started);
    (
        StatusCode::OK,
        Json(SearchResponse {
            matches,
            latency_ms,
            budget_ms: state.budget_ms(),
        }),
    )
        .into_response()
}

async fn stats_handler(State(state): State<IndexState>) -> Response {
    let started = Instant::now();
    let stats = state.stats().await;
    state.record(Method::GET, "/index/stats", StatusCode::OK, started);
    (StatusCode::OK, Json(stats)).into_response()
}

async fn related_handler(
    State(state): State<IndexState>,
    Json(payload): Json<RelatedRequest>,
) -> Response {
    let started = Instant::now();
    let matches = state
        .related(payload.doc_id, payload.k, payload.namespace)
        .await;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    state.record(Method::POST, "/index/related", StatusCode::OK, started);
    (
        StatusCode::OK,
        Json(RelatedResponse {
            matches,
            latency_ms,
            budget_ms: state.budget_ms(),
        }),
    )
        .into_response()
}

async fn forget_handler(
    State(state): State<IndexState>,
    Json(payload): Json<ForgetRequest>,
) -> Response {
    let started = Instant::now();

    // Safety check: require confirmation for non-dry-run
    if !payload.dry_run && !payload.confirm {
        state.record(
            Method::POST,
            "/index/forget",
            StatusCode::BAD_REQUEST,
            started,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Confirmation required for non-dry-run forget operations",
                "hint": "Set 'confirm: true' in the request body"
            })),
        )
            .into_response();
    }

    // Safety check: prevent unfiltered deletion
    // At least one content filter must be specified, OR allow_namespace_wipe must be true
    let has_content_filters = payload.filter.older_than.is_some()
        || payload.filter.source_ref_origin.is_some()
        || payload.filter.doc_id.is_some();

    if !has_content_filters && !payload.filter.allow_namespace_wipe {
        state.record(
            Method::POST,
            "/index/forget",
            StatusCode::BAD_REQUEST,
            started,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "At least one content filter must be specified (older_than, source_ref_origin, doc_id), or set 'allow_namespace_wipe: true' to delete entire namespace",
                "hint": "This safety check prevents accidental deletion of all documents"
            })),
        )
            .into_response();
    }

    // Critical safety check: allow_namespace_wipe requires namespace to be specified
    // This prevents global deletion across ALL namespaces
    if payload.filter.allow_namespace_wipe && payload.filter.namespace.is_none() {
        state.record(
            Method::POST,
            "/index/forget",
            StatusCode::BAD_REQUEST,
            started,
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "allow_namespace_wipe requires namespace to be specified",
                "hint": "To prevent global deletion, namespace must be set when using allow_namespace_wipe"
            })),
        )
            .into_response();
    }

    let result = state.forget(payload.filter, payload.dry_run).await;

    // Log the forget operation
    tracing::info!(
        forgotten_count = result.forgotten_count,
        dry_run = result.dry_run,
        reason = %payload.reason,
        "Forget operation completed"
    );

    state.record(Method::POST, "/index/forget", StatusCode::OK, started);
    (StatusCode::OK, Json(result)).into_response()
}

async fn retention_handler(State(state): State<IndexState>) -> Response {
    let started = Instant::now();
    let configs = state.get_retention_configs().await;
    state.record(Method::GET, "/index/retention", StatusCode::OK, started);
    (StatusCode::OK, Json(RetentionResponse { configs })).into_response()
}

async fn decay_preview_handler(
    State(state): State<IndexState>,
    Json(payload): Json<DecayPreviewRequest>,
) -> Response {
    let started = Instant::now();
    let preview = state.preview_decay(payload.namespace).await;
    state.record(
        Method::POST,
        "/index/decay/preview",
        StatusCode::OK,
        started,
    );
    (StatusCode::OK, Json(preview)).into_response()
}

#[derive(Debug, Deserialize)]
pub struct UpsertRequest {
    pub doc_id: String,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub chunks: Vec<ChunkPayload>,
    #[serde(default)]
    pub meta: Value,
    pub source_ref: Option<SourceRef>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChunkPayload {
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub embedding: Vec<f32>,
    #[serde(default)]
    pub meta: Value,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default)]
    pub k: Option<usize>,
    #[serde(default)]
    pub namespace: Option<String>,
    /// Exclude documents with any of these flags
    /// Default (None): filters PossiblePromptInjection for safety
    /// Empty vec (Some(vec![])): explicitly no filtering
    #[serde(default)]
    pub exclude_flags: Option<Vec<ContentFlag>>,
    /// Minimum trust level required (Low, Medium, High)
    #[serde(default)]
    pub min_trust_level: Option<TrustLevel>,
    /// Exclude documents from these origins
    #[serde(default)]
    pub exclude_origins: Option<Vec<String>>,
}

impl SearchRequest {
    /// Create a basic search request for testing (no security filters)
    #[cfg(test)]
    pub fn test_basic(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            k: None,
            namespace: None,
            exclude_flags: Some(vec![]), // Empty = no filtering
            min_trust_level: None,
            exclude_origins: None,
        }
    }
    
    /// Get the effective exclude_flags with default policy applied
    fn effective_exclude_flags(&self) -> Vec<ContentFlag> {
        match &self.exclude_flags {
            None => vec![ContentFlag::PossiblePromptInjection], // Default policy
            Some(flags) => flags.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RelatedRequest {
    pub doc_id: String,
    #[serde(default)]
    pub k: Option<usize>,
    #[serde(default)]
    pub namespace: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpsertResponse {
    pub status: String,
    pub ingested: usize,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub matches: Vec<SearchMatch>,
    pub latency_ms: f64,
    pub budget_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct RelatedResponse {
    pub matches: Vec<SearchMatch>,
    pub latency_ms: f64,
    pub budget_ms: u64,
}

#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub total_documents: usize,
    pub total_chunks: usize,
    pub namespaces: HashMap<String, usize>,
    pub budget_ms: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchMatch {
    pub doc_id: String,
    pub namespace: String,
    pub chunk_id: String,
    pub score: f32,
    pub text: String,
    pub meta: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    pub ingested_at: String,
    /// Content flags indicating potential security or quality issues
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<ContentFlag>,
}

fn default_namespace() -> String {
    DEFAULT_NAMESPACE.to_string()
}

/// Filter for forgetting documents
#[derive(Debug, Deserialize)]
pub struct ForgetFilter {
    /// Filter by namespace
    #[serde(default)]
    pub namespace: Option<String>,

    /// Filter documents older than this timestamp
    #[serde(default)]
    pub older_than: Option<DateTime<Utc>>,

    /// Filter by source_ref origin
    #[serde(default)]
    pub source_ref_origin: Option<String>,

    /// Filter by specific doc_id
    #[serde(default)]
    pub doc_id: Option<String>,

    /// Explicitly allow wiping entire namespace when only namespace filter is set
    /// This is a safety flag to prevent accidental deletion of all documents in a namespace
    #[serde(default)]
    pub allow_namespace_wipe: bool,
}

/// Request for intentional forgetting
#[derive(Debug, Deserialize)]
pub struct ForgetRequest {
    pub filter: ForgetFilter,
    pub reason: String,
    #[serde(default)]
    pub confirm: bool,
    #[serde(default)]
    pub dry_run: bool,
}

/// Result of a forget operation
#[derive(Debug, Serialize)]
pub struct ForgetResult {
    pub forgotten_count: usize,
    pub dry_run: bool,
    pub forgotten_docs: Vec<ForgottenDocument>,
}

/// Information about a forgotten document
#[derive(Debug, Serialize)]
pub struct ForgottenDocument {
    pub doc_id: String,
    pub namespace: String,
    pub ingested_at: String,
}

/// Response for retention configs listing
#[derive(Debug, Serialize)]
pub struct RetentionResponse {
    pub configs: HashMap<String, RetentionConfig>,
}

/// Request for decay preview
#[derive(Debug, Deserialize)]
pub struct DecayPreviewRequest {
    #[serde(default)]
    pub namespace: Option<String>,
}

/// Response for decay preview
#[derive(Debug, Serialize)]
pub struct DecayPreview {
    pub namespace: String,
    pub total_documents: usize,
    pub previews: Vec<DecayPreviewItem>,
}

/// Individual document's decay preview
#[derive(Debug, Serialize)]
pub struct DecayPreviewItem {
    pub doc_id: String,
    pub namespace: String,
    pub ingested_at: String,
    pub age_seconds: u64,
    pub decay_factor: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;
    use serde_json::json;
    use tower::ServiceExt;
    
    // Helper to create test source refs
    fn test_source_ref(origin: &str, id: &str) -> SourceRef {
        SourceRef {
            origin: origin.to_string(),
            id: id.to_string(),
            offset: None,
            trust_level: TrustLevel::default_for_origin(origin),
            injected_by: None,
        }
    }

    #[tokio::test]
    async fn upsert_and_search_return_ok() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));
        let app = router().with_state(state);

        let payload = serde_json::json!({
            "doc_id": "doc-1",
            "namespace": "default",
            "chunks": [
                {"chunk_id": "doc-1#0", "text": "Hallo Welt", "embedding": []}
            ],
            "meta": {"kind": "markdown"},
            "source_ref": {
                "origin": "chronik",
                "id": "test-event-1",
                "trust_level": "high"
            }
        });

        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/upsert")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);

        let search_payload = serde_json::json!({"query": "Hallo", "k": 1, "namespace": "default"});
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/search")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(search_payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn search_filters_results_by_query() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-rust".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-rust#0".into()),
                    text: Some("Rust programming language".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "rust"}),
                source_ref: Some(test_source_ref("code", "test_file.rs")),
            })
            .await;

        state
            .upsert(UpsertRequest {
                doc_id: "doc-cooking".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-cooking#0".into()),
                    text: Some("A collection of delicious recipes".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "cooking"}),
                source_ref: Some(test_source_ref("user", "recipe-book")),
            })
            .await;

        let results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("default".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "doc-rust");
        assert!(results[0].text.to_lowercase().contains("rust"));
    }

    #[tokio::test]
    async fn trims_namespace_whitespace_on_upsert_and_search() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-trim".into(),
                namespace: "  custom  ".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-trim#0".into()),
                    text: Some("Rust namespaces".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "trim"}),
                source_ref: Some(test_source_ref("chronik", "trim-test")),
            })
            .await;

        let results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("custom".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].namespace, "custom");

        let spaced_results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("   custom   ".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
            })
            .await;

        assert_eq!(spaced_results.len(), 1);
        assert_eq!(spaced_results[0].doc_id, "doc-trim");
    }

    #[tokio::test]
    async fn empty_namespace_defaults_to_default_namespace() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-empty".into(),
                namespace: String::new(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-empty#0".into()),
                    text: Some("Hello default namespace".into()),
                    embedding: Vec::new(),
                    meta: json!({"chunk": 0}),
                }],
                meta: json!({"doc": "empty"}),
                source_ref: Some(test_source_ref("chronik", "empty-test")),
            })
            .await;

        let results = state
            .search(&SearchRequest {
                query: "hello".into(),
                k: Some(5),
                namespace: None,
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].namespace, DEFAULT_NAMESPACE);

        let spaced_results = state
            .search(&SearchRequest {
                query: "hello".into(),
                k: Some(5),
                namespace: Some("   ".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
            })
            .await;

        assert_eq!(spaced_results.len(), 1);
        assert_eq!(spaced_results[0].doc_id, "doc-empty");
        assert_eq!(spaced_results[0].namespace, DEFAULT_NAMESPACE);
    }

    #[tokio::test]
    async fn stats_returns_correct_counts() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-1".into(),
                namespace: "default".into(),
                chunks: vec![
                    ChunkPayload {
                        chunk_id: Some("doc-1#0".into()),
                        text: Some("First chunk".into()),
                        embedding: Vec::new(),
                        meta: json!({}),
                    },
                    ChunkPayload {
                        chunk_id: Some("doc-1#1".into()),
                        text: Some("Second chunk".into()),
                        embedding: Vec::new(),
                        meta: json!({}),
                    },
                ],
                meta: json!({}),
                source_ref: Some(test_source_ref("chronik", "doc-1")),
            })
            .await;

        state
            .upsert(UpsertRequest {
                doc_id: "doc-2".into(),
                namespace: "custom".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-2#0".into()),
                    text: Some("Third chunk".into()),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("chronik", "doc-2")),
            })
            .await;

        let stats = state.stats().await;
        assert_eq!(stats.total_documents, 2);
        assert_eq!(stats.total_chunks, 3);
        assert_eq!(stats.namespaces.len(), 2);
        assert_eq!(stats.namespaces.get("default"), Some(&1));
        assert_eq!(stats.namespaces.get("custom"), Some(&1));
    }

    #[tokio::test]
    async fn related_finds_similar_documents() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}));

        state
            .upsert(UpsertRequest {
                doc_id: "doc-rust".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-rust#0".into()),
                    text: Some("Rust programming language with memory safety".into()),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("code", "rust-doc")),
            })
            .await;

        state
            .upsert(UpsertRequest {
                doc_id: "doc-rust-guide".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-rust-guide#0".into()),
                    text: Some("A guide to memory management in Rust".into()),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("code", "rust-guide")),
            })
            .await;

        state
            .upsert(UpsertRequest {
                doc_id: "doc-python".into(),
                namespace: "default".into(),
                chunks: vec![ChunkPayload {
                    chunk_id: Some("doc-python#0".into()),
                    text: Some("Python scripting tutorial".into()),
                    embedding: Vec::new(),
                    meta: json!({}),
                }],
                meta: json!({}),
                source_ref: Some(test_source_ref("code", "python-doc")),
            })
            .await;

        let related = state
            .related("doc-rust".into(), Some(5), Some("default".into()))
            .await;

        // Should find doc-rust-guide as related (shares "rust" and "memory" words)
        assert!(!related.is_empty());
        assert!(related.iter().any(|m| m.doc_id == "doc-rust-guide"));
    }
}
