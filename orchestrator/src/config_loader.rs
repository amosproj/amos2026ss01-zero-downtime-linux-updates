use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

fn default_cloud() -> String {
    "https://cloud.weber.de/api/v1".into()
}
fn default_interval() -> u32 {
    5
}
fn default_inventory_path() -> String {
    "./inventory/inventory.json".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_cloud")]
    pub cloud_url: String,

    #[serde(default = "default_interval")]
    pub poll_interval_secs: u32,

    #[serde(default = "default_inventory_path")]
    pub inventory_path: String,

    pub https_proxy: Option<String>,

    #[allow(dead_code)]
    pub device_uuid: String,
}

pub fn get_config(config_path: Option<PathBuf>) -> Result<Settings, config::ConfigError> {
    let file_config = match config_path {
        Some(path) => File::from(path).required(true),
        None => File::with_name("config").required(false),
    };
    let env_config = Environment::with_prefix("APP");

    let settings = Config::builder()
        .add_source(file_config)
        .add_source(env_config)
        .build()?;
    let settings: Settings = settings.try_deserialize()?;
    if let Err(err) = validate_config(&settings) {
        return Err(config::ConfigError::Message(err));
    }
    Ok(settings)
}

pub fn validate_config(config: &Settings) -> Result<(), String> {
    if !config.cloud_url.starts_with("https://") && !config.cloud_url.starts_with("http://") {
        return Err("Cloud url must begin with `https://` or `http://`".into());
    }

    if config.poll_interval_secs == 0 {
        return Err("Poll interval must be >= 1 seconds".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_default() -> Settings {
        Settings {
            cloud_url: default_cloud(),
            poll_interval_secs: default_interval(),
            inventory_path: default_inventory_path(),
            https_proxy: None,
            device_uuid: "00000000-0000-0000-0000-000000000000".into(),
        }
    }

    #[test]
    fn url_with_http_succeeds() {
        let mut config = get_default();
        config.cloud_url = "http://weber.cloud/foo".into();

        let validation = validate_config(&config);
        assert!(validation.is_ok());
    }

    #[test]
    fn url_with_https_succeeds() {
        let mut config = get_default();
        config.cloud_url = "https://weber.cloud/foo".into();

        let validation = validate_config(&config);
        assert!(validation.is_ok());
    }

    #[test]
    fn url_without_http_or_https_fails() {
        let mut config = get_default();
        config.cloud_url = "ftp://weber.cloud/foo".into();

        let validation = validate_config(&config);
        assert!(validation.is_err());
    }

    #[test]
    fn poll_interval_zero_fails() {
        let mut config = get_default();
        config.poll_interval_secs = 0;

        let validation = validate_config(&config);
        assert!(validation.is_err());
    }
}
