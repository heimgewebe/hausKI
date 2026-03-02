use serde::{Deserialize, Serialize};

pub const fn default_llm_p95_ms() -> u64 {
    400
}

pub const fn default_index_topk20_ms() -> u64 {
    60
}

pub const fn default_gpu_max_c() -> u64 {
    80
}

pub const fn default_dgpu_power_w() -> u64 {
    220
}

pub const fn default_wer_max_pct() -> u64 {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    #[serde(default)]
    pub latency: Latency,
    #[serde(default)]
    pub thermal: Thermal,
    #[serde(default)]
    pub asr: Asr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Latency {
    #[serde(default = "default_llm_p95_ms")]
    pub llm_p95_ms: u64,
    #[serde(default = "default_index_topk20_ms")]
    pub index_topk20_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Thermal {
    #[serde(default = "default_gpu_max_c")]
    pub gpu_max_c: u64,
    #[serde(default = "default_dgpu_power_w")]
    pub dgpu_power_w: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asr {
    #[serde(default = "default_wer_max_pct")]
    pub wer_max_pct: u64,
}

// NOTE: We keep a manual `Default` implementation here instead of using
// `#[derive(Default)]`. All nested structs provide custom defaults and we want
// this type to stay resilient even if new fields that lack `Default`
// derivations are introduced in the future. The explicit construction also
// makes the intended baseline configuration obvious to readers.
#[allow(clippy::derivable_impls)]
impl Default for Limits {
    fn default() -> Self {
        Self {
            latency: Latency::default(),
            thermal: Thermal::default(),
            asr: Asr::default(),
        }
    }
}

impl Default for Latency {
    fn default() -> Self {
        Self {
            llm_p95_ms: default_llm_p95_ms(),
            index_topk20_ms: default_index_topk20_ms(),
        }
    }
}

impl Default for Thermal {
    fn default() -> Self {
        Self {
            gpu_max_c: default_gpu_max_c(),
            dgpu_power_w: default_dgpu_power_w(),
        }
    }
}

impl Default for Asr {
    fn default() -> Self {
        Self {
            wer_max_pct: default_wer_max_pct(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ModelsFile {
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    pub path: String,
    pub vram_min_gb: Option<u64>,
    pub canary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(transparent)]
pub struct RoutingPolicy(pub serde_yaml_ng::Value);

pub type RoutingRule = serde_yaml_ng::Value;
pub type RoutingDecision = serde_yaml_ng::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct FeatureFlags {
    pub safe_mode: bool,
    pub chat_upstream_url: Option<String>,
    pub chat_model: Option<String>,
    pub events_token: Option<String>,
}
