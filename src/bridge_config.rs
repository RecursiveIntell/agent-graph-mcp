use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Unified configuration schema for proveKV bridge behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Per-application overrides. Key = app name (e.g. "chat", "rag").
    pub apps: HashMap<String, AppConfig>,
    /// Default policy applied when no per-app override exists.
    #[serde(default = "AppConfig::default")]
    pub default: AppConfig,
    /// Global retention settings.
    #[serde(default = "RetentionConfig::default")]
    pub retention: RetentionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Whether state reuse is enabled for this app.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of forked states to retain per capture.
    #[serde(default = "default_max_forks")]
    pub max_forks: usize,
    /// State freshness threshold in seconds. States older than this
    /// are not eligible for reuse.
    #[serde(default = "default_freshness_secs")]
    pub freshness_secs: u64,
    /// Minimum overlap ratio (0.0-1.0) required for fork detection.
    #[serde(default = "default_overlap")]
    pub fork_overlap_min: f64,
}
impl Default for AppConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_forks: 16,
            freshness_secs: 300,
            fork_overlap_min: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetentionConfig {
    /// How long to retain captured states before GC eligibility (seconds).
    #[serde(default = "default_retention_secs")]
    pub retention_secs: u64,
    /// Maximum total states across all apps before aggressive GC.
    #[serde(default = "default_max_states")]
    pub max_total_states: usize,
    /// Maximum disk bytes for KV cache storage.
    #[serde(default = "default_max_bytes")]
    pub max_total_bytes: u64,
}
impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            retention_secs: 3600,
            max_total_states: 10_000,
            max_total_bytes: 1_073_741_824,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_max_forks() -> usize {
    16
}
fn default_freshness_secs() -> u64 {
    300
}
fn default_overlap() -> f64 {
    0.8
}
fn default_retention_secs() -> u64 {
    3600
}
fn default_max_states() -> usize {
    10_000
}
fn default_max_bytes() -> u64 {
    1_073_741_824 // 1 GB
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            apps: HashMap::new(),
            default: AppConfig {
                enabled: true,
                max_forks: 16,
                freshness_secs: 300,
                fork_overlap_min: 0.8,
            },
            retention: RetentionConfig {
                retention_secs: 3600,
                max_total_states: 10_000,
                max_total_bytes: 1_073_741_824,
            },
        }
    }
}

impl BridgeConfig {
    /// Load configuration from a TOML file.
    pub fn from_file(path: &PathBuf) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("failed to read config {}: {e}", path.display()))?;
        toml::from_str(&content).map_err(|e| format!("invalid TOML in {}: {e}", path.display()))
    }

    /// Get the effective config for an app, falling back to default.
    pub fn for_app(&self, app_name: &str) -> &AppConfig {
        self.apps.get(app_name).unwrap_or(&self.default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let cfg = BridgeConfig::default();
        assert!(cfg.default.enabled);
        assert_eq!(cfg.default.max_forks, 16);
        assert!(cfg.retention.max_total_bytes > 0);
    }

    #[test]
    fn app_override_falls_back_to_default() {
        let mut cfg = BridgeConfig::default();
        cfg.apps.insert(
            "chat".into(),
            AppConfig {
                enabled: false,
                max_forks: 16,
                freshness_secs: 300,
                fork_overlap_min: 0.8,
            },
        );
        assert!(!cfg.for_app("chat").enabled);
        assert!(cfg.for_app("rag").enabled); // falls back to default
    }

    #[test]
    fn config_from_toml_string() {
        let toml_str = r#"
[apps.chat]
enabled = false
max_forks = 8

[retention]
retention_secs = 7200
"#;
        let cfg: BridgeConfig = toml::from_str(toml_str).unwrap();
        assert!(!cfg.for_app("chat").enabled);
        assert_eq!(cfg.for_app("chat").max_forks, 8);
        assert_eq!(cfg.retention.retention_secs, 7200);
        assert!(cfg.for_app("rag").enabled); // default
    }
}
