use std::{path::PathBuf, sync::Arc};

use serde_derive::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    cloud_url: String,
    poll_interval_secs: u32,
}

impl ::std::default::Default for Config {
    fn default() -> Self {
        Self {
            cloud_url: String::from("https://weber.cloud/api/bla"),
            poll_interval_secs: 3,
        }
    }
}

pub fn get_config() -> Result<Arc<Config>, confy::ConfyError> {
    let cwd_path = PathBuf::from("config.default.toml");
    let cfg: Config = confy::load_path(cwd_path)?;
    let arc = Arc::new(cfg);
    return Ok(arc);
}
