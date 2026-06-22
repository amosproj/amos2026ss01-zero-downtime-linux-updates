mod config_loader;
use clap::Parser;
use config_loader::get_config;
use tracing::{debug, error, info, warn};

use crate::application::Application;
use crate::loop_os::run_os_tree_main_loop;
use crate::podman::PodmanContainer;
use crate::podman::log_registry::spawn_app_log_registry;
use crate::podman::wrapper::PodmanWrapper;
use crate::state::OsState;
use crate::update_check::{CheckForUpdate, UpdateChecker};
use crate::util::bootc_wrapper::Bootc;
use crate::util::executer::RealExecuter;
use crate::{
    inventory::collect_and_save_inventory, loop_apps::run_apps_main_loop, state::AgentState,
};
mod application;
mod download_manager;
mod healthcheck;
mod inventory;
mod logging;
mod loop_apps;
mod loop_os;
mod podman;
mod state;
mod update_check;
mod util;
use std::env;
use std::path::{Path, PathBuf};
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

    info!("Orchestrator starting ...");

    // Load config first so logging can be tagged with the device UUID. The
    // logging subscriber isn't up yet, so failures go straight to stderr.
    // FIXME: think about how to handle/log failed config reads.
    //   they should be logged and sent to remote db but also include the device UUID
    //   depens on how UUID will be provided in the future.
    let config = Arc::new(get_config(cli.config.clone()).unwrap_or_else(|err| {
        eprintln!("Failed to load config: {err}");
        std::process::exit(1);
    }));

    let log_rx = logging::init(cli.debug);

    let signer = match util::tpm::tpm_init() {
        Ok(signer) => signer,
        Err(err) => {
            error!("TPM init failed: {}", err);
            std::process::exit(1);
        }
    };

    let bootc_client = Arc::new(Bootc::new(Box::new(RealExecuter)));

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

    info!(
        version = VERSION,
        device_uuid = %config.device_uuid,
        "Orchestrator started",
    );
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
    let bootc_status = bootc_client.status().await.unwrap_or_else(|err| {
        error!("Failed to fetch initial bootc status: {}", err);
        std::process::exit(1);
    });
    let os_state = OsState {
        update_pending: bootc_status.staged.is_some(),
        booted_image: bootc_status.booted.unwrap().checksum.clone(),
        update_ostree_commit: bootc_status.staged.map(|s| s.checksum),
    };

    let download_manager = Arc::new(
        match download_manager::DownloadManager::new(Arc::clone(&config), signer) {
            Ok(dm) => dm,
            Err(err) => {
                error!("Failed to initialize secure cloud HTTP client: {:?}", err);
                std::process::exit(1);
            }
        },
    );

    info!("Log shipper starting");
    logging::spawn_log_shipper(log_rx, Arc::clone(&download_manager));
    info!("Log shipper started");

    info!("Application log registry starting");
    let app_log_registry = spawn_app_log_registry(Arc::clone(&download_manager));
    info!("Application log registry started");

    info!("Reading inital application state");
    let (podman, containers) = PodmanWrapper::connect(Path::new(&config.podman_path))
        .await
        .unwrap();

    // Idk how else to get application ids other than this
    let target_configs = download_manager
        .get_target_application_configs()
        .await
        .unwrap_or_default();
    let (matched, unmatched) = loop_apps::resolve_application_ids(containers, &target_configs);

    for c in &unmatched {
        warn!(
            container = c.name(),
            "No matching application config for recovered container"
        );
    }

    let apps_state = matched
        .into_iter()
        .map(|(c, id)| Application::wrap(c, id, &app_log_registry))
        .collect();
    debug!("Creating AgentState");
    let agent_state = AgentState::new(VERSION, Arc::clone(&config), os_state, apps_state);

    info!(
        "Running amos-zero-downtime with version: {}",
        agent_state.self_version
    );

    let update_checker: Arc<dyn CheckForUpdate> =
        Arc::new(UpdateChecker::new(Arc::clone(&download_manager)));

    let _apps_handle = tokio::spawn(run_apps_main_loop(
        agent_state.clone(),
        podman,
        Arc::clone(&download_manager),
        app_log_registry.clone(),
    ));
    let _os_tree_handle = tokio::spawn(run_os_tree_main_loop(
        agent_state.clone(),
        Arc::clone(&bootc_client),
        Arc::clone(&download_manager),
        Arc::clone(&update_checker),
    ));

    let mut healthcheck_interval = tokio::time::interval(std::time::Duration::from_secs(60));
    let download_manager_clone = Arc::clone(&download_manager);
    let _health_report_handle = tokio::spawn(async move {
        loop {
            healthcheck_interval.tick().await;
            if let Err(err) = download_manager_clone.send_ping().await {
                warn!("Aliveness report failed: {}", err);
            }
        }
    });

    info!("Background workers spawned, orchestrator is running");

    tokio::signal::ctrl_c().await.unwrap();
    info!("Received shutdown signal, stopping");
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
