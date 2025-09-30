use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    pub latency: Latency,
    pub thermal: Thermal,
    pub asr: Asr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Latency {
    pub llm_p95_ms: u64,
    pub index_topk20_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thermal {
    pub gpu_max_c: u64,
    pub dgpu_power_w: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asr {
    pub wer_max_pct: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsFile {
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub path: String,
    pub vram_min_gb: Option<u64>,
    pub canary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct RoutingPolicy(pub serde_yaml::Value);

pub type RoutingRule = serde_yaml::Value;
pub type RoutingDecision = serde_yaml::Value;

pub fn load_limits<P: AsRef<Path>>(path: P) -> anyhow::Result<Limits> {
    let content = fs::read_to_string(path)?;
    let limits = serde_yaml::from_str(&content)?;
    Ok(limits)
}

pub fn load_models<P: AsRef<Path>>(path: P) -> anyhow::Result<ModelsFile> {
    let content = fs::read_to_string(path)?;
    let models = serde_yaml::from_str(&content)?;
    Ok(models)
}

pub fn load_routing<P: AsRef<Path>>(path: P) -> anyhow::Result<RoutingPolicy> {
    let content = fs::read_to_string(path)?;
    let routing = serde_yaml::from_str(&content)?;
    Ok(routing)
}
