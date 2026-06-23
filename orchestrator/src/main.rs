//! This program runs on the IPC and connects to the cloud via an API.
//! From there, it pulls the desired state and tries to make the local configuration conform to it.

mod config;
use anyhow::Context;
use clap::Parser;
use tracing::{debug, error, info};

use crate::api_client::ApiClient;
use crate::application::Application;
use crate::config::OrchestratorConfig;
use crate::logging::OrchestratorLogger;
use crate::loop_os::{OsState, run_os_main_loop};
use crate::loop_ping::run_ping_main_loop;
use crate::podman::log_registry::spawn_app_log_registry;
use crate::podman::wrapper::PodmanWrapper;
use crate::util::bootc_wrapper::Bootc;
use crate::util::device_jwt::DeviceJwtProvider;
use crate::util::executer::RealExecuter;

use crate::loop_apps::run_apps_main_loop;
use crate::util::tpm::TpmSigner;
mod api_client;
mod application;
mod logging;
mod loop_apps;
mod loop_os;
mod loop_ping;
mod podman;
mod util;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

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

    if cli.self_check {
        if let Err(e) = self_check(&cli).await {
            error!("{:?}", e.context("Self check failed"));
            std::process::exit(1);
        }
    } else {
        if let Err(e) = run(&cli).await {
            error!("{:?}", e);
            std::process::exit(1);
        }
    }
}

async fn run(cli: &Cli) -> anyhow::Result<()> {
    let logger = OrchestratorLogger::init(cli.debug);

    info!("Orchestrator starting ...");

    let config =
        OrchestratorConfig::load(cli.config.as_deref()).context("Could not load configuration")?;
    debug!("Loaded config: {:?}", config);

    let signer = TpmSigner::new().context("Could not initialize the TPM")?;
    let jwt_provider = DeviceJwtProvider::new(signer);
    let api_client = Arc::new(
        ApiClient::new(
            config.https_proxy,
            config.cloud_url,
            config.device_uuid.clone(),
            jwt_provider,
        )
        .context("Could not initialize the api client")?,
    );

    let log_shipper_task = logger.into_spawned_shipper(
        api_client.clone(),
        Duration::from_secs(config.log_flush_interval_secs),
        config.log_max_batch,
        config.log_max_buffer,
    );

    let app_log_registry = spawn_app_log_registry(
        api_client.clone(),
        Duration::from_secs(config.log_flush_interval_secs),
        config.log_max_batch,
        config.log_max_buffer,
    );

    let bootc = Bootc::new(Box::new(RealExecuter));
    let os_state = OsState::new(bootc.status().await?)
        .ok_or(anyhow::anyhow!("Could not retrieve current OS state"))?;

    let (podman, containers) = PodmanWrapper::connect(Path::new(&config.podman_path))
        .await
        .context("Could not initialize connection to Podman")?;
    let apps = containers
        .into_iter()
        .map(|c| Application::wrap(c, 0, &app_log_registry))
        .collect();

    let poll_interval = Duration::from_secs(config.poll_interval_secs as u64);
    let apps_task = tokio::spawn(run_apps_main_loop(
        apps,
        podman,
        api_client.clone(),
        poll_interval,
        app_log_registry,
    ));
    let os_task = tokio::spawn(run_os_main_loop(
        os_state,
        bootc,
        api_client.clone(),
        poll_interval,
    ));
    let ping_task = tokio::spawn(run_ping_main_loop(api_client, Duration::from_secs(60)));

    info!(
        version = VERSION,
        device_uuid = %config.device_uuid,
        "Orchestrator started",
    );

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = log_shipper_task => {},
        _ = apps_task => {},
        _ = os_task => {},
        _ = ping_task => {}
    }

    Ok(())
}

async fn self_check(cli: &Cli) -> anyhow::Result<()> {
    OrchestratorLogger::init(cli.debug);
    let config = OrchestratorConfig::load(cli.config.as_deref())?;

    let bootc = Bootc::new(Box::new(RealExecuter));
    bootc.status().await?;

    // TODO: Maybe check if container can start properly?
    PodmanWrapper::connect(Path::new(&config.podman_path)).await?;

    Ok(())
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
