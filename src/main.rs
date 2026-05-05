mod config_loader;
use config_loader::{get_config, validate_config};
use log::{debug, info};

use crate::{apps::get_initial_state, os_tree::get_inital_os_state, state::AgentState};
mod apps;
mod os_tree;
mod state;

const VERSION: &str = "0.0.1";

#[tokio::main]
async fn main() {
    println!("Started app...");

    let config = get_config().unwrap_or_else(|err| {
        eprintln!("Failed to load config: {}", err);
        std::process::exit(1);
    });

    validate_config(&config).unwrap_or_else(|err| {
        eprintln!("Failed during config validation: {}", err);
        std::process::exit(1);
    });

    println!("Loaded config: {:?}", config);

    info!("Reading inital OS State");
    let os_state = get_inital_os_state();

    info!("Reading inital application state");
    let apps_state = get_initial_state();

    debug!("Creating AgentState");
    let agent_state = AgentState::new(VERSION, os_state, apps_state);

    // TODO: start os and apps loop, give them a copy of agent state each

    tokio::signal::ctrl_c().await.unwrap();
}
