mod config_loader;
use config_loader::{get_config, validate_config};
use log::{debug, error, info};

use crate::{
    apps::{get_initial_apps_state, run_apps_main_loop},
    inventory::collect_and_save_inventory,
    os_tree::{get_inital_os_state, run_os_tree_main_loop},
    state::AgentState,
};
mod apps;
mod inventory;
mod os_tree;
mod state;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() {
    env_logger::init();

    info!("Started app...");

    let config = get_config().unwrap_or_else(|err| {
        error!("Failed to load config: {}", err);
        std::process::exit(1);
    });

    validate_config(&config).unwrap_or_else(|err| {
        error!("Failed during config validation: {}", err);
        std::process::exit(1);
    });

    debug!("Loaded config: {:?}", config);

    info!("Collecting initial inventory");
    if let Err(err) =
        collect_and_save_inventory(std::path::Path::new(config.inventory_path.as_str()))
    {
        error!("Failed to collect and save inventory: {}", err);
        std::process::exit(1);
    }

    info!("Reading inital OS State");
    let os_state = get_inital_os_state();

    info!("Reading inital application state");
    let apps_state = get_initial_apps_state();

    debug!("Creating AgentState");
    let agent_state = AgentState::new(VERSION, config, os_state, apps_state);

    info!(
        "Running amos-zero-downtime with version: {}",
        agent_state.self_version
    );

    // TODO: start os and apps loop, give them a copy of agent state each
    let _apps_handle = tokio::spawn(run_apps_main_loop(agent_state.clone()));
    let _os_tree_handle = tokio::spawn(run_os_tree_main_loop(agent_state.clone()));
    tokio::signal::ctrl_c().await.unwrap();
}
