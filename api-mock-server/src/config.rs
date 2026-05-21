use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

fn default_database_url() -> String {
    "postgres://app:4M0S@127.0.0.1:5432/amos".into()
}
fn default_http_port() -> u16 {
    8080
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_database_url")]
    pub database_url: String,

    #[serde(default = "default_http_port")]
    pub http_port: u16,
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
    if !config.database_url.starts_with("postgres://")
        && !config.database_url.starts_with("sqlite:")
    {
        return Err("Database connection url must begin with `postgres://`".into());
    }

    if config.http_port == 0 {
        return Err("HTTP port must be > 0".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_default() -> Result<Settings, config::ConfigError> {
        let config = Config::builder().build()?;
        config.try_deserialize()
    }

    #[test]
    fn jdbc_url_without_postgres_fails() {
        let mut config = get_default().unwrap();
        config.database_url = "mysql://foo:bar@baz:bum".into();

        let validation = validate_config(&config);
        assert!(validation.is_err());
    }

    #[test]
    fn http_port_0_fails() {
        let mut config = get_default().unwrap();
        config.http_port = 0;

        let validation = validate_config(&config);
        assert!(validation.is_err());
    }
}
