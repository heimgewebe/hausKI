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
#[serde(deny_unknown_fields)]
pub struct ModelEntry {
    pub id: String,
    pub path: String,
    pub vram_min_gb: Option<u64>,
    pub canary: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingPolicy {
    pub default: RoutingDecision,
    #[serde(default)]
    pub allow: Vec<RoutingRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingDecision {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingRule {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

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

#[cfg(test)]
mod tests {
    use super::{load_routing, RoutingDecision};
    use serial_test::serial;
    use std::fs;

    #[test]
    #[serial]
    fn parses_default_deny_with_empty_allow_list() {
        let path = std::env::temp_dir().join("hauski-routing-policy-test.yaml");
        let _ = fs::remove_file(&path);

        fs::write(&path, "default: deny\nallow: []\n").expect("policy file written");

        let policy = load_routing(&path).expect("policy parsed");
        assert!(policy.allow.is_empty());
        assert!(matches!(policy.default, RoutingDecision::Deny));

        fs::remove_file(&path).ok();
    }
}
