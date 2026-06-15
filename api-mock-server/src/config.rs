use config::{Config, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;

fn default_database_url() -> String {
    "postgres://app:4M0S@127.0.0.1:5432/amos".into()
}
fn default_timescale_database_url() -> String {
    "postgres://app:4M0S@127.0.0.1:5433/amos_timeseries".into()
}
fn default_http_port() -> u16 {
    8080
}

fn default_subject_claim() -> String {
    "sub".into()
}

fn default_name_claim() -> String {
    "name".into()
}

fn default_public_key() -> String {
    "-----BEGIN PUBLIC KEY-----
MIICIjANBgkqhkiG9w0BAQEFAAOCAg8AMIICCgKCAgEAzP3Oc7fe4hRq7wMKxyfS
wiQclOzJIvoTLB0Tnxy6sEqUcg7WFV1Xcw25DuzIj6ZIlGhKIr6jKs+8G1rLymTZ
tIdJEx2wcKTPfTezth2/nMT9E2Dct0Q9aM2Yi/LUyVBmGD3Go14KoXA8EZbDOQOW
0wMREw5qsim6gI5Jm9O2XUUFwS+U28CoSqMKFNlJFdZodqa6mVsTQG6gmdtMbjyG
kX8KjEPcShNTZePWFimk3hBuBwSLtsYG2Ws2eyVYTbYPuI9Prmbfboykm/L9OYFZ
ZNyC/2bv7P9jJWIv6dwByKhcBZBHCxTYiPkTuxzN51JMyJA4okCPDoNJJRai5top
4oWtf7VQJHyKHUIpCZNMUD9bo+wBccvsd+o9WcQg/l5JRuKYipz61tiwKdbExPUh
RC6SgfDNg5YIPadLNbA+NGeFeXQtn+PYExcGkAcB/hbS6Ppj1Het67zuOGOZF8SE
/HzbIAQ4lHcOLCXVfGrXwB7DvhYYgQ3DAypvVS67fyggzcule2jcTGbrGpjb4YIk
eOovQuaa/ks1ymihNl18iJYZEDr/o/OhMiWaWOrLp/vnHeCIubgX6N1hMwopqgen
Anm8E788IHh9EybwO/uEiDqfSXlR8cmeBhD3B+vrkjbCnz/p6o8nhOzVabJUkYGa
RwsluOuHZzbXjtbwKS9rJ5sCAwEAAQ==
-----END PUBLIC KEY-----
"
    .into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct JwtConfig {
    pub subject_claim: String,

    pub name_claim: String,

    pub public_key: String,
}

impl Default for JwtConfig {
    fn default() -> Self {
        JwtConfig {
            subject_claim: default_subject_claim(),
            name_claim: default_name_claim(),
            public_key: default_public_key(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Settings {
    #[serde(default = "default_database_url")]
    pub database_url: String,

    #[serde(default = "default_timescale_database_url")]
    pub timescale_database_url: String,

    #[serde(default = "default_http_port")]
    pub http_port: u16,

    #[serde(default)]
    pub jwt: JwtConfig,
}

pub fn get_config(config_path: Option<PathBuf>) -> Result<Settings, config::ConfigError> {
    let file_config = match config_path {
        Some(path) => File::from(path).required(true),
        None => File::with_name("config").required(false),
    };

    // Default separator is "." which is a PITA when working with Unix shells...
    let env_config = Environment::with_prefix("APP")
        .separator("__")
        .prefix_separator("_");

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

    if !config.timescale_database_url.starts_with("postgres://") {
        return Err("TimescaleDB connection url must begin with `postgres://`".into());
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
    fn timescale_url_without_postgres_fails() {
        let mut config = get_default().unwrap();
        config.timescale_database_url = "sqlite::memory:".into();

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
