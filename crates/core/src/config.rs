use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const fn default_llm_p95_ms() -> u64 {
    400
}

const fn default_index_topk20_ms() -> u64 {
    60
}

const fn default_gpu_max_c() -> u64 {
    80
}

const fn default_dgpu_power_w() -> u64 {
    220
}

const fn default_wer_max_pct() -> u64 {
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
pub struct RoutingPolicy(pub serde_yaml::Value);

pub type RoutingRule = serde_yaml::Value;
pub type RoutingDecision = serde_yaml::Value;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct FeatureFlags {
    pub safe_mode: bool,
}

fn parse_env_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn load_limits<P: AsRef<Path>>(path: P) -> anyhow::Result<Limits> {
    let path = path.as_ref();
    match fs::read_to_string(path) {
        Ok(content) => match serde_yaml::from_str(&content) {
            Ok(limits) => Ok(limits),
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "failed to parse limits YAML, falling back to defaults"
                );
                Ok(Limits::default())
            }
        },
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "failed to read limits YAML, falling back to defaults"
            );
            Ok(Limits::default())
        }
    }
}

pub fn load_models<P: AsRef<Path>>(path: P) -> anyhow::Result<ModelsFile> {
    let path = path.as_ref();
    match fs::read_to_string(path) {
        Ok(content) => match serde_yaml::from_str(&content) {
            Ok(models) => Ok(models),
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "failed to parse models YAML, falling back to defaults"
                );
                Ok(ModelsFile::default())
            }
        },
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "failed to read models YAML, falling back to defaults"
            );
            Ok(ModelsFile::default())
        }
    }
}

pub fn load_routing<P: AsRef<Path>>(path: P) -> anyhow::Result<RoutingPolicy> {
    let path = path.as_ref();
    match fs::read_to_string(path) {
        Ok(content) => match serde_yaml::from_str(&content) {
            Ok(routing) => Ok(routing),
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "failed to parse routing YAML, falling back to defaults"
                );
                Ok(RoutingPolicy::default())
            }
        },
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "failed to read routing YAML, falling back to defaults"
            );
            Ok(RoutingPolicy::default())
        }
    }
}

pub fn load_flags<P: AsRef<Path>>(path: P) -> anyhow::Result<FeatureFlags> {
    let path = path.as_ref();
    let mut flags = match fs::read_to_string(path) {
        Ok(content) => match serde_yaml::from_str(&content) {
            Ok(flags) => flags,
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "failed to parse flags YAML, falling back to defaults"
                );
                FeatureFlags::default()
            }
        },
        Err(err) => {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "failed to read flags YAML, falling back to defaults"
            );
            FeatureFlags::default()
        }
    };

    if let Ok(value) = std::env::var("HAUSKI_SAFE_MODE") {
        match parse_env_bool(&value) {
            Some(parsed) => {
                flags.safe_mode = parsed;
            }
            None => {
                tracing::warn!(
                    invalid_value = %value,
                    "invalid boolean for HAUSKI_SAFE_MODE, keeping configured value"
                );
            }
        }
    }

    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs::File;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_limits_file_falls_back_to_defaults() {
        let limits = load_limits("/does/not/exist.yaml").unwrap();
        assert_eq!(limits.latency.llm_p95_ms, default_llm_p95_ms());
        assert_eq!(limits.latency.index_topk20_ms, default_index_topk20_ms());
    }

    #[test]
    fn partial_yaml_merges_with_defaults() {
        let path = std::env::temp_dir().join(format!(
            "hauski-test-limits-{}.yaml",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "latency:\n  llm_p95_ms: 350\n").unwrap();
            file.flush().unwrap();
        }

        let limits = load_limits(&path).unwrap();
        assert_eq!(limits.latency.llm_p95_ms, 350);
        assert_eq!(limits.latency.index_topk20_ms, default_index_topk20_ms());
        assert_eq!(limits.thermal.gpu_max_c, default_gpu_max_c());
        assert_eq!(limits.asr.wer_max_pct, default_wer_max_pct());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn routing_policy_with_explicit_default_and_no_rules() {
        let path = std::env::temp_dir().join(format!(
            "hauski-test-routing-{}.yaml",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "default: deny").unwrap();
        }

        let routing = load_routing(&path).unwrap();
        let mapping = routing
            .0
            .as_mapping()
            .expect("routing policy should be a mapping");
        let default_key = serde_yaml::Value::String("default".into());
        let allow_key = serde_yaml::Value::String("allow".into());
        assert_eq!(
            mapping.get(&default_key),
            Some(&serde_yaml::Value::String("deny".into()))
        );
        assert!(!mapping.contains_key(&allow_key));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn missing_models_file_falls_back_to_empty_list() {
        let models = load_models("/does/not/exist/models.yaml").unwrap();
        assert!(models.models.is_empty());
    }

    #[test]
    fn invalid_models_yaml_falls_back_to_defaults() {
        let path = std::env::temp_dir().join("hauski-test-invalid-models.yaml");
        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "models: not-a-list").unwrap();
            file.flush().unwrap();
        }

        let models = load_models(&path).unwrap();
        assert!(models.models.is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_routing_yaml_falls_back_to_defaults() {
        let path = std::env::temp_dir().join("hauski-test-invalid-routing.yaml");
        {
            let mut file = File::create(&path).unwrap();
            write!(file, "routing: [invalid").unwrap();
            file.flush().unwrap();
        }

        let routing = load_routing(&path).unwrap();
        assert_eq!(routing, RoutingPolicy::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn missing_flags_file_defaults_to_safe_mode_off() {
        let flags = load_flags("/does/not/exist-flags.yaml").unwrap();
        assert!(!flags.safe_mode);
    }

    #[serial]
    #[test]
    fn env_override_wins_over_file() {
        let path = std::env::temp_dir().join(format!(
            "hauski-test-flags-{}.yaml",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "safe_mode: false").unwrap();
        }

        std::env::set_var("HAUSKI_SAFE_MODE", "true");
        let flags = load_flags(&path).unwrap();
        assert!(flags.safe_mode);
        std::env::remove_var("HAUSKI_SAFE_MODE");
        let _ = std::fs::remove_file(path);
    }

    #[serial]
    #[test]
    fn invalid_env_override_keeps_config() {
        let path = std::env::temp_dir().join(format!(
            "hauski-test-flags-invalid-env-{}.yaml",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let mut file = File::create(&path).unwrap();
            writeln!(file, "safe_mode: true").unwrap();
        }

        std::env::set_var("HAUSKI_SAFE_MODE", "definitely-not-a-bool");
        let flags = load_flags(&path).unwrap();
        assert!(flags.safe_mode);
        std::env::remove_var("HAUSKI_SAFE_MODE");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn parse_env_bool_accepts_common_truthy_and_falsy_values() {
        for truthy in ["1", "true", "TRUE", " yes ", "On"] {
            assert_eq!(parse_env_bool(truthy), Some(true), "truthy: {truthy:?}");
        }

        for falsy in ["0", "false", "FALSE", " no ", "off"] {
            assert_eq!(parse_env_bool(falsy), Some(false), "falsy: {falsy:?}");
        }
    }

    #[test]
    fn parse_env_bool_rejects_invalid_values() {
        for invalid in ["", "maybe", "10", "enable"] {
            assert_eq!(parse_env_bool(invalid), None, "invalid: {invalid:?}");
        }
    }
}
