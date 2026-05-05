use config::{Config, Environment, File};
use serde::Deserialize;

fn default_cloud() -> String { "https://cloud.weber.de/api/v1".into() }
fn default_interval() -> u32 { 5 }

#[derive(Debug, Deserialize)]
pub struct Settings {
    #[serde(default = "default_cloud")]
    cloud_url: String,

    #[serde(default = "default_interval")]
    poll_interval_secs: u32,
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    let file_config = File::with_name("config").required(false);
    let env_config = Environment::with_prefix("APP");

    let settings = Config::builder()
        .add_source(file_config)
        .add_source(env_config)
        .build()?;

    return settings.try_deserialize()
}

pub fn validate_config(config: &Settings) -> Result<(), String> {
    if !config.cloud_url.starts_with("https://") {
        return Err("Cloud url must begin with `https://`".into());
    }

    if config.poll_interval_secs <= 0 {
        return Err("Poll interval must be >= 1 seconds".into());
    }

    Ok(())
}
