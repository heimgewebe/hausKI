use axum::{
    extract::{FromRef, State},
    http::{Method, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use chrono::{DateTime, Utc};
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram::Histogram;
use prometheus_client::registry::Registry;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    borrow::Cow,
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    io,
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};
use thiserror::Error;
use tokio::sync::RwLock;
use ulid::Ulid;

const DEFAULT_NAMESPACE: &str = "default";
const QUARANTINE_NAMESPACE: &str = "quarantine";
const MIN_WORD_LENGTH_FOR_SIMILARITY: usize = 3;
const WORD_MATCH_SCORE_INCREMENT: f32 = 0.1;

// Decision feedback storage limits
const MAX_DECISION_SNAPSHOTS: usize = 10_000;
const MAX_DECISION_OUTCOMES: usize = 10_000;
const SNAPSHOT_CANDIDATES_MAX: usize = 50;

pub type MetricsRecorder = dyn Fn(Method, &'static str, StatusCode, Instant) + Send + Sync;

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct WeightFactorLabels {
    factor: String, // "trust", "recency", "context"
}

#[derive(Debug, Error)]
enum PolicyLoadError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

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
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TrustLevel {
    /// Low trust - external sources, user input, tool output
    Low,
    /// Medium trust - OS context, application logs
    Medium,
    /// High trust - chronik events, verified internal sources
    High,
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::Low => write!(f, "low"),
            TrustLevel::Medium => write!(f, "medium"),
            TrustLevel::High => write!(f, "high"),
        }
    }
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

// Policy definitions
pub trait ValidatePolicy {
    fn validate(&self) -> Result<(), String>;
}

/// Policy defining trust levels and their weights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustPolicy {
    /// Mapping of trust levels ("high", "medium", "low") to weight multipliers.
    pub trust_weights: BTreeMap<String, f32>, // BTreeMap for stable hash
    /// Minimum weight floor to prevent total suppression.
    #[serde(default = "default_min_weight")]
    pub min_weight: f32,
}

impl ValidatePolicy for TrustPolicy {
    fn validate(&self) -> Result<(), String> {
        if self.min_weight <= 0.0 {
            return Err("min_weight must be > 0".to_string());
        }
        for (level, weight) in &self.trust_weights {
            if *weight <= 0.0 {
                return Err(format!("Trust weight for '{}' must be > 0", level));
            }
        }
        // Ensure required keys exist
        for required in &["high", "medium", "low"] {
            if !self.trust_weights.contains_key(*required) {
                return Err(format!("Missing required trust level: {}", required));
            }
        }

        // Ensure min_weight doesn't accidentally suppress configured weights
        for (level, weight) in &self.trust_weights {
            if *weight < self.min_weight {
                return Err(format!(
                    "Trust weight for '{}' ({}) is less than min_weight ({}). This would be silently clamped.",
                    level, weight, self.min_weight
                ));
            }
        }
        Ok(())
    }
}

impl Default for TrustPolicy {
    fn default() -> Self {
        let mut trust_weights = BTreeMap::new();
        trust_weights.insert("high".to_string(), 1.0);
        trust_weights.insert("medium".to_string(), 0.7);
        trust_weights.insert("low".to_string(), 0.3);
        Self {
            trust_weights,
            min_weight: 0.1,
        }
    }
}

fn default_min_weight() -> f32 {
    0.1
}

/// Policy defining context-based weighting profiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    /// Profiles defining namespace/origin weights.
    pub profiles: BTreeMap<String, BTreeMap<String, f32>>, // BTreeMap for stable hash (outer and inner)
    /// Recency decay configuration.
    pub recency: RecencyPolicy,
}

impl ValidatePolicy for ContextPolicy {
    fn validate(&self) -> Result<(), String> {
        self.recency.validate()?;
        for (profile_name, weights) in &self.profiles {
            for (namespace, weight) in weights {
                if *weight <= 0.0 {
                    return Err(format!(
                        "Context weight for '{}/{}' must be > 0",
                        profile_name, namespace
                    ));
                }
            }
            if !weights.contains_key("_default") {
                return Err(format!(
                    "Profile '{}' is missing required '_default' key (fallback weight)",
                    profile_name
                ));
            }
        }
        if !self.profiles.contains_key("default") {
            return Err("Missing required 'default' profile".to_string());
        }
        Ok(())
    }
}

impl Default for ContextPolicy {
    fn default() -> Self {
        let mut default_profile = BTreeMap::new();
        default_profile.insert("_default".to_string(), 1.0);

        let mut profiles = BTreeMap::new();
        profiles.insert("default".to_string(), default_profile);

        Self {
            profiles,
            recency: RecencyPolicy::default(),
        }
    }
}

/// Configuration for time-based decay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecencyPolicy {
    /// Half-life in seconds for exponential decay.
    pub default_half_life_seconds: u64,
    /// Minimum weight after decay.
    #[serde(default = "default_min_weight")]
    pub min_weight: f32,
}

impl ValidatePolicy for RecencyPolicy {
    fn validate(&self) -> Result<(), String> {
        if self.min_weight <= 0.0 {
            return Err("recency.min_weight must be > 0".to_string());
        }
        // half_life can be 0 (no decay), but usually > 0
        Ok(())
    }
}

impl Default for RecencyPolicy {
    fn default() -> Self {
        Self {
            default_half_life_seconds: 604800, // 7 days
            min_weight: 0.1,
        }
    }
}

/// Aggregated policy configuration loaded at runtime.
#[derive(Clone)]
pub struct PolicyConfig {
    pub trust: TrustPolicy,
    pub context: ContextPolicy,
    /// Stable hash of the loaded policies for drift detection.
    pub hash: String,
    /// Source of the policy configuration (e.g. "loaded_from_disk", "fallback_defaults").
    pub source: String,
}

struct IndexInner {
    store: RwLock<HashMap<String, NamespaceStore>>,
    metrics: Arc<MetricsRecorder>,
    budget_ms: u64,
    retention_configs: RwLock<HashMap<String, RetentionConfig>>,
    policies: PolicyConfig,
    // Prometheus metrics
    prom_weight_applied: Family<WeightFactorLabels, Counter>,
    prom_score_bucket: Histogram,
    // Decision feedback storage
    decision_snapshots: RwLock<HashMap<String, DecisionSnapshot>>,
    decision_outcomes: RwLock<HashMap<String, DecisionOutcome>>,
    // Decision metrics
    prom_decision_snapshots_total: Counter,
    prom_decision_outcomes_total: Family<OutcomeLabels, Counter>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct OutcomeLabels {
    outcome: String, // "success", "failure", "neutral"
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
    pub fn new(
        budget_ms: u64,
        metrics: Arc<MetricsRecorder>,
        registry: Option<&mut Registry>,
        policy_paths: Option<(PathBuf, PathBuf)>, // (trust_path, context_path)
    ) -> Self {
        // Load policies or use defaults
        let (trust_policy, context_policy, policy_hash, policy_source) = if let Some((
            trust_path,
            context_path,
        )) = policy_paths
        {
            // Attempt to load trust policy
            let (trust, trust_source) = match Self::load_policy::<TrustPolicy>(&trust_path) {
                Ok(p) => (p, "file"),
                Err(e) => {
                    tracing::error!(path = %trust_path.display(), error = %e, "Failed to load trust policy, falling back to default");
                    (TrustPolicy::default(), "fallback")
                }
            };

            // Attempt to load context policy
            let (context, context_source) = match Self::load_policy::<ContextPolicy>(&context_path)
            {
                Ok(p) => (p, "file"),
                Err(e) => {
                    tracing::error!(path = %context_path.display(), error = %e, "Failed to load context policy, falling back to default");
                    (ContextPolicy::default(), "fallback")
                }
            };

            // Compute stable hash of policies
            let mut hasher = Sha256::new();
            hasher.update(
                serde_json::to_vec(&trust).expect("Failed to serialize trust policy for hashing"),
            );
            hasher.update(
                serde_json::to_vec(&context)
                    .expect("Failed to serialize context policy for hashing"),
            );
            let hash = format!("{:x}", hasher.finalize());

            let source = if trust_source == "file" && context_source == "file" {
                "loaded_from_disk".to_string()
            } else if trust_source == "fallback" && context_source == "fallback" {
                "fallback_defaults".to_string()
            } else {
                "partial_fallback".to_string()
            };

            (trust, context, hash, source)
        } else {
            (
                TrustPolicy::default(),
                ContextPolicy::default(),
                "default".to_string(),
                "defaults_no_config".to_string(),
            )
        };

        tracing::info!(
            policy_hash = %policy_hash,
            policy_source = %policy_source,
            "Decision weighting policies initialized"
        );

        // Initialize Prometheus metrics
        let prom_weight_applied = Family::<WeightFactorLabels, Counter>::default();
        // Custom buckets for score distribution (0.0 to 2.0+, weighted towards top)
        let prom_score_bucket = Histogram::new([
            0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.85, 0.9, 0.95, 0.99, 1.0, 1.2, 1.5, 2.0,
        ]);

        // Decision feedback metrics
        let prom_decision_snapshots_total = Counter::default();
        let prom_decision_outcomes_total = Family::<OutcomeLabels, Counter>::default();

        if let Some(registry) = registry {
            registry.register(
                "decision_weight_applied",
                "Total number of times a decision weight factor was applied",
                prom_weight_applied.clone(),
            );
            registry.register(
                "decision_final_score",
                "Distribution of final weighted scores",
                prom_score_bucket.clone(),
            );
            registry.register(
                "decision_snapshots_total",
                "Total number of decision snapshots emitted",
                prom_decision_snapshots_total.clone(),
            );
            registry.register(
                "decision_outcomes_total",
                "Total number of decision outcomes reported",
                prom_decision_outcomes_total.clone(),
            );
        }

        Self {
            inner: Arc::new(IndexInner {
                store: RwLock::new(HashMap::new()),
                metrics,
                budget_ms,
                retention_configs: RwLock::new(HashMap::new()),
                policies: PolicyConfig {
                    trust: trust_policy,
                    context: context_policy,
                    hash: policy_hash,
                    source: policy_source,
                },
                prom_weight_applied,
                prom_score_bucket,
                decision_snapshots: RwLock::new(HashMap::new()),
                decision_outcomes: RwLock::new(HashMap::new()),
                prom_decision_snapshots_total,
                prom_decision_outcomes_total,
            }),
        }
    }

    fn load_policy<T: for<'de> Deserialize<'de> + Default + ValidatePolicy>(
        path: &Path,
    ) -> Result<T, PolicyLoadError> {
        let content = std::fs::read_to_string(path).map_err(PolicyLoadError::Io)?;
        let policy: T = serde_yaml_ng::from_str(&content).map_err(PolicyLoadError::Yaml)?;
        policy.validate().map_err(PolicyLoadError::Validation)?;
        Ok(policy)
    }

    /// Helper to get weight for a trust level from policy
    fn get_trust_weight(&self, trust_level: TrustLevel) -> f32 {
        let key = trust_level.to_string();
        let min_weight = self.inner.policies.trust.min_weight;

        // Policy validation ensures all keys exist.
        // If not found (shouldn't happen with valid policy), fallback to hardcoded default for safety.
        let weight = self
            .inner
            .policies
            .trust
            .trust_weights
            .get(&key)
            .cloned()
            .unwrap_or(match trust_level {
                TrustLevel::High => 1.0,
                TrustLevel::Medium => 0.7,
                TrustLevel::Low => 0.3,
            });

        // Apply minimum floor defined in policy
        weight.max(min_weight)
    }

    /// Helper to get context weight from policy
    ///
    /// Strategy:
    /// 1. Look up weight by `namespace`. If present and != 1.0, it wins (Topology).
    /// 2. If namespace is "default" or its weight is 1.0 (neutral), look up `origin`. If present, it wins (Semantics).
    /// 3. Fallback to profile `_default`.
    fn get_context_weight(
        &self,
        namespace: &str,
        source_ref: Option<&SourceRef>,
        profile_name: Option<&str>,
    ) -> f32 {
        let profile_name = profile_name.unwrap_or("default");
        let profile = match self.inner.policies.context.profiles.get(profile_name) {
            Some(p) => p,
            None => {
                if profile_name != "default" {
                    tracing::warn!(profile = %profile_name, "Requested context profile not found, falling back to default");
                }
                match self.inner.policies.context.profiles.get("default") {
                    Some(p) => p,
                    None => return 1.0,
                }
            }
        };

        // 1. Check namespace
        let ns_weight = profile
            .get(namespace)
            .filter(|&&w| (w - 1.0).abs() > f32::EPSILON);

        // 2. Check origin
        let origin_weight = if let Some(sr) = source_ref {
            profile
                .get(&sr.origin)
                .filter(|&&w| (w - 1.0).abs() > f32::EPSILON)
        } else {
            None
        };

        // Decision logic:
        // - Namespace explicit (non-neutral) wins.
        // - Origin (non-neutral) wins.
        // - Profile default wins.
        // - 1.0.

        if let Some(&w) = ns_weight {
            return w;
        }

        if let Some(&w) = origin_weight {
            return w;
        }

        *profile.get("_default").unwrap_or(&1.0)
    }

    pub fn policy_hash(&self) -> &str {
        &self.inner.policies.hash
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
        let namespace_store = store
            .entry(target_namespace.clone())
            .or_insert_with(HashMap::new);
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

        // Use recency policy default if no specific retention config
        let recency_policy = &self.inner.policies.context.recency;

        // Prepare filter criteria (use typed enums, not strings)
        let exclude_flags_set = request.effective_exclude_flags();
        let min_trust = request.min_trust_level;
        let exclude_origins_set: Vec<String> = request.exclude_origins.clone().unwrap_or_default();

        let mut matches: Vec<SearchMatch> = Vec::new();
        let mut filtered_count = 0;
        // Track if any weight factors were actually non-neutral (applied) during this search
        let mut trust_applied = false;
        let mut recency_applied = false;
        let mut context_applied = false;

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
            let has_excluded_flag = doc
                .flags
                .iter()
                .any(|flag| exclude_flags_set.contains(flag));
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

                // Calculate trust weight from source_ref
                // Default to Medium trust if source_ref is missing for safety
                let trust_level = doc
                    .source_ref
                    .as_ref()
                    .map(|sr| sr.trust_level)
                    .unwrap_or(TrustLevel::Medium);

                let trust_weight = self.get_trust_weight(trust_level);

                // Calculate recency weight (time-decay) if configured
                // Clamp age to 0 to handle future timestamps gracefully (clock skew)
                // Use retention config if available, otherwise policy default
                let age_seconds = (now - doc.ingested_at).num_seconds().max(0);
                let half_life = retention_config
                    .and_then(|c| c.half_life_seconds)
                    .unwrap_or(recency_policy.default_half_life_seconds);

                let recency_weight = calculate_decay_factor(age_seconds, Some(half_life))
                    .max(recency_policy.min_weight);

                // Calculate context weight based on namespace and profile
                let context_weight = self.get_context_weight(
                    &doc.namespace,
                    doc.source_ref.as_ref(),
                    request.context_profile.as_deref(),
                );

                // Apply decision weighting: final_score = similarity × trust × recency × context
                let final_score = base_score * trust_weight * recency_weight * context_weight;

                // Track if factors are active (non-neutral)
                if (trust_weight - 1.0).abs() > f32::EPSILON {
                    trust_applied = true;
                }
                if (recency_weight - 1.0).abs() > f32::EPSILON {
                    recency_applied = true;
                }
                if (context_weight - 1.0).abs() > f32::EPSILON {
                    context_applied = true;
                }

                // Optionally include weight breakdown for transparency
                let weights = if request.include_weights {
                    Some(WeightBreakdown {
                        similarity: base_score,
                        trust: trust_weight,
                        recency: recency_weight,
                        context: context_weight,
                    })
                } else {
                    None
                };

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
                    weights,
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

        // Update metrics (per search, not per match, to reduce volume)
        if !matches.is_empty() {
            // Count factors once per search ONLY if they were actually applied (non-neutral)
            if trust_applied {
                self.inner
                    .prom_weight_applied
                    .get_or_create(&WeightFactorLabels {
                        factor: "trust".into(),
                    })
                    .inc();
            }
            if recency_applied {
                self.inner
                    .prom_weight_applied
                    .get_or_create(&WeightFactorLabels {
                        factor: "recency".into(),
                    })
                    .inc();
            }
            if context_applied {
                self.inner
                    .prom_weight_applied
                    .get_or_create(&WeightFactorLabels {
                        factor: "context".into(),
                    })
                    .inc();
            }

            // Observe distribution only for top result to keep histogram volume manageable
            self.inner
                .prom_score_bucket
                .observe(matches[0].score.into());
        }

        // Audit Logging (Debug level, structured)
        if tracing::enabled!(tracing::Level::DEBUG) {
            // Log full breakdown and check for ranking changes ONLY if weights are available
            if request.include_weights {
                tracing::debug!(
                    query = %request.query,
                    matches = ?matches.iter().take(3).map(|m| (&m.doc_id, m.score, &m.weights)).collect::<Vec<_>>(),
                    "Decision weighting breakdown"
                );

                // Check for ranking changes
                // Find "raw top" (max similarity) without cloning/sorting
                // This is O(N) and allocation-free
                let raw_top = matches
                    .iter()
                    .max_by(|a, b| match (&a.weights, &b.weights) {
                        (Some(wa), Some(wb)) => wa
                            .similarity
                            .partial_cmp(&wb.similarity)
                            .unwrap_or(Ordering::Equal),
                        _ => Ordering::Equal,
                    });

                if let Some(top) = raw_top {
                    // matches is sorted by final score, so matches[0] is weighted_top
                    if !matches.is_empty() && top.doc_id != matches[0].doc_id {
                        tracing::debug!(
                            query = %request.query,
                            original_top = %top.doc_id,
                            weighted_top = %matches[0].doc_id,
                            "Decision weighting changed top result"
                        );
                    }
                }
            }
        }

        // Emit decision snapshot if explicitly requested
        // This is decoupled from include_weights (which is just for response transparency)
        if request.emit_decision_snapshot && !matches.is_empty() {
            let decision_id = Ulid::new().to_string();

            // Build candidates list from matches, capped at SNAPSHOT_CANDIDATES_MAX
            // We only need top-N plus a few near-misses for learning
            let candidates: Vec<DecisionCandidate> = matches
                .iter()
                .take(SNAPSHOT_CANDIDATES_MAX)
                .map(|m| {
                    let weights = m.weights.clone().unwrap_or(WeightBreakdown {
                        similarity: m.score,
                        trust: 1.0,
                        recency: 1.0,
                        context: 1.0,
                    });

                    DecisionCandidate {
                        id: m.doc_id.clone(),
                        similarity: weights.similarity,
                        weights,
                        final_score: m.score,
                    }
                })
                .collect();

            let candidates_count = candidates.len();

            let snapshot = DecisionSnapshot {
                decision_id: decision_id.clone(),
                intent: request.query.clone(),
                timestamp: Utc::now().to_rfc3339(),
                namespace: namespace.to_string(),
                context_profile: request.context_profile.clone(),
                candidates,
                selected_id: Some(matches[0].doc_id.clone()),
                policy_hash: self.inner.policies.hash.clone(),
            };

            // Store snapshot with capacity management
            let mut snapshots = self.inner.decision_snapshots.write().await;

            // If at capacity, remove oldest snapshot (ULID is time-sortable)
            if snapshots.len() >= MAX_DECISION_SNAPSHOTS {
                if let Some(oldest_id) = snapshots.keys().min().cloned() {
                    snapshots.remove(&oldest_id);
                    tracing::debug!(
                        oldest_id = %oldest_id,
                        "Removed oldest decision snapshot (capacity limit reached)"
                    );
                }
            }

            snapshots.insert(decision_id.clone(), snapshot);

            // Update metrics
            self.inner.prom_decision_snapshots_total.inc();

            tracing::debug!(
                decision_id = %decision_id,
                candidates_count = candidates_count,
                selected_id = %matches[0].doc_id,
                "Decision snapshot emitted"
            );
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
            policy_hash: Some(self.inner.policies.hash.clone()),
            policy_source: Some(self.inner.policies.source.clone()),
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
                        weights: None, // related() doesn't use decision weighting
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

    /// Get a decision snapshot by ID
    pub async fn get_decision_snapshot(&self, decision_id: &str) -> Option<DecisionSnapshot> {
        let snapshots = self.inner.decision_snapshots.read().await;
        snapshots.get(decision_id).cloned()
    }

    /// List all decision snapshots (for heimlern consumption)
    /// Returns snapshots sorted by decision_id (which is ULID, so time-ordered)
    pub async fn list_decision_snapshots(&self) -> Vec<DecisionSnapshot> {
        let snapshots = self.inner.decision_snapshots.read().await;
        let mut snapshots_vec: Vec<DecisionSnapshot> = snapshots.values().cloned().collect();
        snapshots_vec.sort_by(|a, b| a.decision_id.cmp(&b.decision_id));
        snapshots_vec
    }

    /// Record an outcome for a decision
    ///
    /// hausKI validates the schema and stores the outcome, but does NOT
    /// interpret or act on it. That's heimlern's responsibility.
    pub async fn record_outcome(&self, outcome: DecisionOutcome) -> Result<(), IndexError> {
        // Validate that the decision_id exists
        let snapshots = self.inner.decision_snapshots.read().await;
        if !snapshots.contains_key(&outcome.decision_id) {
            return Err(IndexError {
                error: format!("Decision ID {} not found", outcome.decision_id),
                code: "decision_not_found".into(),
                details: Some(serde_json::json!({
                    "hint": "Decision snapshot must exist before recording outcome"
                })),
            });
        }
        drop(snapshots);

        // Store outcome with capacity management
        let mut outcomes = self.inner.decision_outcomes.write().await;

        // If at capacity, remove oldest outcome (ULID is time-sortable)
        if outcomes.len() >= MAX_DECISION_OUTCOMES {
            if let Some(oldest_id) = outcomes.keys().min().cloned() {
                outcomes.remove(&oldest_id);
                tracing::debug!(
                    oldest_id = %oldest_id,
                    "Removed oldest decision outcome (capacity limit reached)"
                );
            }
        }

        outcomes.insert(outcome.decision_id.clone(), outcome.clone());

        // Update metrics
        self.inner
            .prom_decision_outcomes_total
            .get_or_create(&OutcomeLabels {
                outcome: outcome.outcome.to_string(),
            })
            .inc();

        tracing::info!(
            decision_id = %outcome.decision_id,
            outcome = %outcome.outcome,
            source = %outcome.signal_source,
            "Decision outcome recorded"
        );

        Ok(())
    }

    /// Get an outcome for a decision
    pub async fn get_decision_outcome(&self, decision_id: &str) -> Option<DecisionOutcome> {
        let outcomes = self.inner.decision_outcomes.read().await;
        outcomes.get(decision_id).cloned()
    }

    /// List all decision outcomes (for heimlern consumption)
    /// Returns outcomes sorted by decision_id (which is ULID, so time-ordered)
    pub async fn list_decision_outcomes(&self) -> Vec<DecisionOutcome> {
        let outcomes = self.inner.decision_outcomes.read().await;
        let mut outcomes_vec: Vec<DecisionOutcome> = outcomes.values().cloned().collect();
        outcomes_vec.sort_by(|a, b| a.decision_id.cmp(&b.decision_id));
        outcomes_vec
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
        .route(
            "/decisions/snapshot",
            axum::routing::get(list_decision_snapshots_handler),
        )
        .route(
            "/decisions/snapshot/{id}",
            axum::routing::get(get_decision_snapshot_handler),
        )
        .route("/decisions/outcome", post(record_outcome_handler))
        .route(
            "/decisions/outcome/{id}",
            axum::routing::get(get_decision_outcome_handler),
        )
        .route(
            "/decisions/outcomes",
            axum::routing::get(list_decision_outcomes_handler),
        )
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
            state.record(
                Method::POST,
                "/index/upsert",
                StatusCode::UNPROCESSABLE_ENTITY,
                started,
            );
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

async fn list_decision_snapshots_handler(State(state): State<IndexState>) -> Response {
    let started = Instant::now();
    let snapshots = state.list_decision_snapshots().await;
    state.record(
        Method::GET,
        "/index/decisions/snapshot",
        StatusCode::OK,
        started,
    );
    (
        StatusCode::OK,
        Json(DecisionSnapshotsResponse { snapshots }),
    )
        .into_response()
}

async fn get_decision_snapshot_handler(
    State(state): State<IndexState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let started = Instant::now();
    match state.get_decision_snapshot(&id).await {
        Some(snapshot) => {
            state.record(
                Method::GET,
                "/index/decisions/snapshot/:id",
                StatusCode::OK,
                started,
            );
            (StatusCode::OK, Json(snapshot)).into_response()
        }
        None => {
            state.record(
                Method::GET,
                "/index/decisions/snapshot/:id",
                StatusCode::NOT_FOUND,
                started,
            );
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Decision snapshot not found",
                    "decision_id": id
                })),
            )
                .into_response()
        }
    }
}

async fn record_outcome_handler(
    State(state): State<IndexState>,
    Json(mut payload): Json<DecisionOutcome>,
) -> Response {
    let started = Instant::now();

    // Set timestamp if not provided
    if payload.timestamp.is_empty() {
        payload.timestamp = Utc::now().to_rfc3339();
    }

    match state.record_outcome(payload).await {
        Ok(()) => {
            state.record(
                Method::POST,
                "/index/decisions/outcome",
                StatusCode::OK,
                started,
            );
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "recorded"
                })),
            )
                .into_response()
        }
        Err(error) => {
            state.record(
                Method::POST,
                "/index/decisions/outcome",
                StatusCode::UNPROCESSABLE_ENTITY,
                started,
            );
            (StatusCode::UNPROCESSABLE_ENTITY, Json(error)).into_response()
        }
    }
}

async fn get_decision_outcome_handler(
    State(state): State<IndexState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let started = Instant::now();
    match state.get_decision_outcome(&id).await {
        Some(outcome) => {
            state.record(
                Method::GET,
                "/index/decisions/outcome/:id",
                StatusCode::OK,
                started,
            );
            (StatusCode::OK, Json(outcome)).into_response()
        }
        None => {
            state.record(
                Method::GET,
                "/index/decisions/outcome/:id",
                StatusCode::NOT_FOUND,
                started,
            );
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Decision outcome not found",
                    "decision_id": id
                })),
            )
                .into_response()
        }
    }
}

async fn list_decision_outcomes_handler(State(state): State<IndexState>) -> Response {
    let started = Instant::now();
    let outcomes = state.list_decision_outcomes().await;
    state.record(
        Method::GET,
        "/index/decisions/outcomes",
        StatusCode::OK,
        started,
    );
    (StatusCode::OK, Json(DecisionOutcomesResponse { outcomes })).into_response()
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
    /// Context profile for weighting (e.g., "incident_response", "code_analysis", "reflection")
    /// If None, uses default balanced weighting (1.0 for all namespaces)
    #[serde(default)]
    pub context_profile: Option<String>,
    /// Include weight breakdown in response for transparency
    #[serde(default)]
    pub include_weights: bool,
    /// Emit a decision snapshot for this search (for heimlern learning)
    /// Independent of include_weights - this explicitly controls snapshot emission
    #[serde(default)]
    pub emit_decision_snapshot: bool,
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
            context_profile: None,
            include_weights: false,
            emit_decision_snapshot: false,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_source: Option<String>,
}

/// Weight breakdown for decision-making transparency
#[derive(Debug, Serialize, Clone)]
pub struct WeightBreakdown {
    /// Base similarity score (0.0 - 1.0)
    pub similarity: f32,
    /// Trust weight multiplier based on source trust level
    pub trust: f32,
    /// Recency weight based on document age (exponential decay)
    pub recency: f32,
    /// Context weight based on namespace and intent
    pub context: f32,
}

#[derive(Debug, Serialize, Clone)]
pub struct SearchMatch {
    pub doc_id: String,
    pub namespace: String,
    pub chunk_id: String,
    /// Final weighted score (similarity × trust × recency × context)
    pub score: f32,
    pub text: String,
    pub meta: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<SourceRef>,
    pub ingested_at: String,
    /// Content flags indicating potential security or quality issues
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<ContentFlag>,
    /// Optional weight breakdown for transparency (only included when requested)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weights: Option<WeightBreakdown>,
}

// ---- Decision Feedback Structures --------------------------------------------

/// A candidate considered during a decision
#[derive(Debug, Serialize, Clone)]
pub struct DecisionCandidate {
    /// Document ID of the candidate
    pub id: String,
    /// Base similarity score (0.0 - 1.0)
    pub similarity: f32,
    /// Weight factors applied to this candidate
    pub weights: WeightBreakdown,
    /// Final weighted score (similarity × trust × recency × context)
    pub final_score: f32,
}

/// Decision snapshot for feedback and learning
///
/// This captures everything about a weighted decision so that heimlern
/// can analyze and learn from it without hausKI interpreting outcomes.
#[derive(Debug, Serialize, Clone)]
pub struct DecisionSnapshot {
    /// Unique decision identifier (ULID)
    pub decision_id: String,
    /// The intent/query that triggered this decision
    pub intent: String,
    /// Timestamp when the decision was made
    pub timestamp: String,
    /// Namespace searched
    pub namespace: String,
    /// Context profile used (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_profile: Option<String>,
    /// All candidates considered (sorted by final_score, descending)
    pub candidates: Vec<DecisionCandidate>,
    /// ID of the selected candidate (top result)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_id: Option<String>,
    /// Policy hash at time of decision (for drift detection)
    pub policy_hash: String,
}

/// Outcome signal for a decision
///
/// This is hausKI's API for accepting feedback. hausKI validates and stores
/// but does NOT interpret or act on this feedback. That's heimlern's job.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeSignal {
    /// Decision led to successful outcome
    Success,
    /// Decision led to unsuccessful outcome
    Failure,
    /// Outcome was neither clearly good nor bad
    Neutral,
}

impl std::fmt::Display for OutcomeSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutcomeSignal::Success => write!(f, "success"),
            OutcomeSignal::Failure => write!(f, "failure"),
            OutcomeSignal::Neutral => write!(f, "neutral"),
        }
    }
}

/// Source of outcome feedback
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeSource {
    /// User-provided feedback
    User,
    /// System-generated feedback
    System,
    /// Policy-based feedback
    Policy,
}

impl std::fmt::Display for OutcomeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutcomeSource::User => write!(f, "user"),
            OutcomeSource::System => write!(f, "system"),
            OutcomeSource::Policy => write!(f, "policy"),
        }
    }
}

/// Decision outcome record
///
/// hausKI accepts this via API, validates the schema, and stores it.
/// hausKI does NOT interpret or act on this data.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DecisionOutcome {
    /// Decision ID this outcome refers to
    pub decision_id: String,
    /// Outcome signal
    pub outcome: OutcomeSignal,
    /// Source of the feedback
    pub signal_source: OutcomeSource,
    /// Timestamp when feedback was recorded
    pub timestamp: String,
    /// Optional context or notes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
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

/// Response containing list of decision snapshots
#[derive(Debug, Serialize)]
pub struct DecisionSnapshotsResponse {
    pub snapshots: Vec<DecisionSnapshot>,
}

/// Response containing list of decision outcomes
#[derive(Debug, Serialize)]
pub struct DecisionOutcomesResponse {
    pub outcomes: Vec<DecisionOutcome>,
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
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);
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
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

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
            .await
            .expect("upsert should succeed");

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
            .await
            .expect("upsert should succeed");

        let results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("default".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
                context_profile: None,
                include_weights: false,
                emit_decision_snapshot: false,
            })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].doc_id, "doc-rust");
        assert!(results[0].text.to_lowercase().contains("rust"));
    }

    #[tokio::test]
    async fn trims_namespace_whitespace_on_upsert_and_search() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

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
            .await
            .expect("upsert should succeed");

        let results = state
            .search(&SearchRequest {
                query: "rust".into(),
                k: Some(5),
                namespace: Some("custom".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
                context_profile: None,
                include_weights: false,
                emit_decision_snapshot: false,
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
                context_profile: None,
                include_weights: false,
                emit_decision_snapshot: false,
            })
            .await;

        assert_eq!(spaced_results.len(), 1);
        assert_eq!(spaced_results[0].doc_id, "doc-trim");
    }

    #[tokio::test]
    async fn empty_namespace_defaults_to_default_namespace() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

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
            .await
            .expect("upsert should succeed");

        let results = state
            .search(&SearchRequest {
                query: "hello".into(),
                k: Some(5),
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
        assert_eq!(results[0].namespace, DEFAULT_NAMESPACE);

        let spaced_results = state
            .search(&SearchRequest {
                query: "hello".into(),
                k: Some(5),
                namespace: Some("   ".into()),
                exclude_flags: None,
                min_trust_level: None,
                exclude_origins: None,
                context_profile: None,
                include_weights: false,
                emit_decision_snapshot: false,
            })
            .await;

        assert_eq!(spaced_results.len(), 1);
        assert_eq!(spaced_results[0].doc_id, "doc-empty");
        assert_eq!(spaced_results[0].namespace, DEFAULT_NAMESPACE);
    }

    #[tokio::test]
    async fn stats_returns_correct_counts() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

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
            .await
            .expect("upsert should succeed");

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
            .await
            .expect("upsert should succeed");

        let stats = state.stats().await;
        assert_eq!(stats.total_documents, 2);
        assert_eq!(stats.total_chunks, 3);
        assert_eq!(stats.namespaces.len(), 2);
        assert_eq!(stats.namespaces.get("default"), Some(&1));
        assert_eq!(stats.namespaces.get("custom"), Some(&1));
    }

    #[tokio::test]
    async fn related_finds_similar_documents() {
        let state = IndexState::new(60, Arc::new(|_, _, _, _| {}), None, None);

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
            .await
            .expect("upsert should succeed");

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
            .await
            .expect("upsert should succeed");

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
            .await
            .expect("upsert should succeed");

        let related = state
            .related("doc-rust".into(), Some(5), Some("default".into()))
            .await;

        // Should find doc-rust-guide as related (shares "rust" and "memory" words)
        assert!(!related.is_empty());
        assert!(related.iter().any(|m| m.doc_id == "doc-rust-guide"));
    }
}
