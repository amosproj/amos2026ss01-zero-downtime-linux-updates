use std::path::PathBuf;

use anyhow::{Result, anyhow};

use util::bootc_wrapper::Bootc;
use util::executer::Executer;

use crate::{config_loader::get_config, inventory::healthcheck_inventory};

pub async fn healthcheck(
    bootc: &Bootc,
    exec: &dyn Executer,
    config_path: Option<PathBuf>,
) -> Result<()> {
    let inventory_result = healthcheck_inventory(bootc, exec).await;
    let config_result = get_config(config_path).map(|_| ());

    match (inventory_result, config_result) {
        (Ok(()), Ok(())) => Ok(()),
        (inventory_err, config_err) => Err(anyhow!(format!(
            "Inventory healthcheck: {} | Config healthcheck: {}",
            inventory_err
                .err()
                .map(|err| err.to_string())
                .unwrap_or_else(|| "ok".to_string()),
            config_err
                .err()
                .map(|err| err.to_string())
                .unwrap_or_else(|| "ok".to_string())
        ))),
    }
}
