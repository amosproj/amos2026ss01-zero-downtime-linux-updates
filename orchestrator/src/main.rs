mod config_loader;
use clap::Parser;
use config_loader::get_config;
use log::{debug, error, info, warn};

use crate::loop_os::run_os_tree_main_loop;
use crate::util::bootc_wrapper::Bootc;
use crate::util::executer::RealExecuter;
use crate::util::os_tree::RpmOstreeClient;

use crate::{
    inventory::collect_and_save_inventory,
    loop_apps::{get_initial_apps_state, run_apps_main_loop},
    state::AgentState,
};
mod download_manager;
mod healthcheck;
mod inventory;
mod loop_apps;
mod loop_os;
mod state;
mod util;
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "Orchestrator")]
#[command(version = VERSION)]
#[command(about = "Orchestrates bootc/os-tree and application container updates", long_about = None)]
struct Cli {
    /// If the self check should be run instead of the main programm loop
    #[arg(short, long)]
    pub self_check: bool,

    /// Sets a custom config file path
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Adjust log level according to verbosity specified via CLI
    let mut log_level = log::LevelFilter::Warn;
    for _ in 0..cli.debug {
        log_level = log_level.increment_severity();
    }

    env_logger::builder().filter_level(log_level).init();

    let executer = RealExecuter;
    let bootc_client = Bootc::new(Box::new(executer));

    let ostree_client = Arc::new(RpmOstreeClient::new(Arc::new(RealExecuter)));

    // run the selfcheck pipeline if --self-check is provided as commandline arg
    if cli.self_check {
        if let Err(err) =
            crate::healthcheck::healthcheck(&bootc_client, &RealExecuter, cli.config.clone()).await
        {
            error!("Self check failed: {}", err);
            std::process::exit(1);
        }
        info!("Self check passed");
        std::process::exit(0);
    }

    info!("Started app...");

    let config = get_config(cli.config).unwrap_or_else(|err| {
        error!("Failed to load config: {}", err);
        std::process::exit(1);
    });

    debug!("Loaded config: {:?}", config);

    info!("Collecting initial inventory");
    if let Err(err) = collect_and_save_inventory(
        &bootc_client,
        &RealExecuter,
        std::path::Path::new(config.inventory_path.as_str()),
    )
    .await
    {
        error!("Failed to collect and save inventory: {}", err);
        std::process::exit(1);
    }

    info!("Reading inital OS State");
    let os_state = ostree_client.status().await.unwrap_or_else(|err| {
        error!("Failed to fetch initial rpm-ostree status: {}", err);
        std::process::exit(1);
    });

    info!("Reading inital application state");
    let apps_state = get_initial_apps_state();

    debug!("Creating AgentState");
    let agent_state = AgentState::new(VERSION, config, os_state, apps_state);

    info!(
        "Running amos-zero-downtime with version: {}",
        agent_state.self_version
    );

    let http_client = match download_manager::build_http_client_from_settings(&config) {
        Ok(client) => client,
        Err(err) => {
            error!("Failed to initialize secure cloud HTTP client: {:?}", err);
            std::process::exit(1);
        }
    };

    info!("Polling cloud server for targeted system updates...");
    match download_manager::check_for_update(&http_client, &dm_config).await {
        Ok(sync_response) => {
            info!(
                "Cloud sync successful. Server response: {:?}",
                sync_response
            );

            if let Some(target_hash) = sync_response.target_os_commit_hash {
                if target_hash != dm_config.current_os_version {
                    info!(
                        "New target OS update discovered [Hash: {}]. Starting download phase...",
                        target_hash
                    );

                    match download_manager::download_update(&http_client, &target_hash, &dm_config)
                        .await
                    {
                        Ok(downloaded_path) => {
                            info!(
                                "Update image successfully staged locally at: {:?}",
                                downloaded_path
                            );
                            // downloaded_path to bootc here
                        }
                        Err(err) => {
                            error!(
                                "Critical failure while transferring update payload: {:?}",
                                err
                            );
                        }
                    }
                } else {
                    info!("System is up to date. Target matches running commit hash.");
                }
            } else {
                info!(
                    "No target OS updates assigned to this hardware profile by cloud orchestrator."
                );
            }
        }
        Err(err) => {
            warn!("Cloud synchronization check failed: {:?}", err);
        }
    }

    let _apps_handle = tokio::spawn(run_apps_main_loop(agent_state.clone()));
    let _os_tree_handle = tokio::spawn(run_os_tree_main_loop(
        agent_state.clone(),
        ostree_client.clone(),
    ));

    tokio::signal::ctrl_c().await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::Parser;
    use std::path::PathBuf;

    #[test]
    fn parse_self_check_flag() {
        let cli = Cli::parse_from(["orchestrator", "--self-check"]);
        assert!(cli.self_check);
    }

    #[test]
    fn parse_config_path() {
        let cli = Cli::parse_from(["orchestrator", "--config", "/tmp/config.toml"]);
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/config.toml")));
    }

    #[test]
    fn parse_debug_count() {
        let cli = Cli::parse_from(["orchestrator", "-dd"]);
        assert_eq!(cli.debug, 2);
    }
}
