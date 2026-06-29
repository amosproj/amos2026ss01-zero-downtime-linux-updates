//! Define and load configuration from different sources

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OrchestratorConfig {
    pub cloud_url: String,
    pub poll_interval_secs: u32,
    pub podman_path: String,
    pub https_proxy: Option<String>,

    pub log_flush_interval_secs: u64,
    pub log_max_batch: usize,
    pub log_max_buffer: usize,

    pub deferred_update_timer_secs: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            cloud_url: "https://cloud.weber.de/v1".to_owned(),
            poll_interval_secs: 5,
            podman_path: "/run/podman/podman.sock".to_owned(),
            https_proxy: None,

            log_flush_interval_secs: 60,
            log_max_batch: 256,
            log_max_buffer: 10_000,

            deferred_update_timer_secs: 43200, // 12 hours default
        }
    }
}

impl OrchestratorConfig {
    /// Loads and validates the orchestrator configuration.
    ///
    /// The config file is resolved in order of precedence:
    /// 1. `config_path` (the `--config` CLI flag), required if `Some`.
    /// 2. The `APP_CONFIG_FILE` environment variable, required if set.
    /// 3. `config.toml` in the current working directory (optional).
    ///
    /// Individual settings can additionally be overridden with `APP_`-prefixed
    /// environment variables (e.g. `APP_CLOUD_URL`), which take precedence over
    /// the file.
    pub fn load(path: Option<&Path>) -> anyhow::Result<Self> {
        let mut cb = config::Config::builder();

        // Get config file path from input, environment or convention
        cb = if let Some(config_path) = path {
            cb.add_source(config::File::from(config_path))
        } else if let Ok(config_path) = std::env::var("APP_CONFIG_FILE") {
            cb.add_source(config::File::from(config_path.as_ref()))
        } else {
            cb.add_source(config::File::with_name("config").required(false))
        };

        // Read environment as last resort before default
        cb = cb.add_source(config::Environment::with_prefix("APP"));

        let result: OrchestratorConfig = cb.build()?.try_deserialize()?;
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> anyhow::Result<()> {
        if !self.cloud_url.starts_with("https://") && !self.cloud_url.starts_with("http://") {
            anyhow::bail!("Cloud URL must begin with `https://` or `http://`");
        }

        if self.poll_interval_secs == 0 {
            anyhow::bail!("Poll interval must be >= 1 seconds");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_with_http_succeeds() {
        let mut config = OrchestratorConfig::default();
        config.cloud_url = "http://weber.cloud/foo".to_owned();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn url_with_https_succeeds() {
        let mut config = OrchestratorConfig::default();
        config.cloud_url = "https://weber.cloud/foo".to_owned();

        assert!(config.validate().is_ok());
    }

    #[test]
    fn url_without_http_or_https_fails() {
        let mut config = OrchestratorConfig::default();
        config.cloud_url = "ftp://weber.cloud/foo".into();

        assert!(config.validate().is_err());
    }

    #[test]
    fn poll_interval_zero_fails() {
        let mut config = OrchestratorConfig::default();
        config.poll_interval_secs = 0;

        assert!(config.validate().is_err());
    }
}
