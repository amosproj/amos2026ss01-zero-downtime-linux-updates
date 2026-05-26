use crate::state::{AgentState, OsState};
use anyhow::{Context, Result, anyhow};

use log::{error, info};
use std::sync::Arc;

use std::time::Duration;
use tokio::time::interval;

use crate::util::executer::Executer;

pub struct RpmOstreeClient {
    executer: Arc<dyn Executer>,
}

impl RpmOstreeClient {
    pub fn new(executer: Arc<dyn Executer>) -> Self {
        Self { executer }
    }

    pub async fn status(&self) -> Result<OsState> {
        let res = self
            .executer
            .execute(
                "sudo".to_string(),
                vec![
                    "rpm-ostree".to_string(),
                    "status".to_string(),
                    "--json".to_string(),
                ],
            )
            .await?;

        if res.exit_code != Some(0) {
            return Err(anyhow!(
                "rpm-ostree status failed with stderr: {}",
                res.stderr
            ));
        }

        let status: OsState = serde_json::from_str(&res.stdout)
            .context("Failed to parse rpm-ostree status JSON payload")?;
        Ok(status)
    }

    pub async fn upgrade(&self) -> Result<()> {
        let res = self
            .executer
            .execute(
                "sudo".to_string(),
                vec!["rpm-ostree".to_string(), "upgrade".to_string()],
            )
            .await?;

        if res.exit_code != Some(0) {
            return Err(anyhow!("rpm-ostree upgrade failed: {}", res.stderr));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn rebase(&self, target_reference: &str) -> Result<()> {
        let res = self
            .executer
            .execute(
                "sudo".to_string(),
                vec![
                    "rpm-ostree".to_string(),
                    "rebase".to_string(),
                    target_reference.to_string(),
                ],
            )
            .await?;

        if res.exit_code != Some(0) {
            return Err(anyhow!("rpm-ostree rebase failed: {}", res.stderr));
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn rollback(&self) -> Result<()> {
        let res = self
            .executer
            .execute(
                "sudo".to_string(),
                vec!["rpm-ostree".to_string(), "rollback".to_string()],
            )
            .await?;

        // Exit code 137 can happen if systemd begins terminating tasks immediately
        match res.exit_code {
            Some(0) | Some(137) => Ok(()),
            _ => Err(anyhow!(
                "rpm-ostree rollback failed with exit code: {:?}",
                res.exit_code
            )),
        }
    }

    pub async fn apply_reboot(&self) -> Result<()> {
        let res = self
            .executer
            .execute(
                "sudo".to_string(),
                vec!["systemctl".to_string(), "reboot".to_string()],
            )
            .await?;

        match res.exit_code {
            Some(0) | Some(137) => Ok(()),
            _ => Err(anyhow!(
                "systemctl reboot invocation failed with exit code: {:?}",
                res.exit_code
            )),
        }
    }
}

pub async fn run_os_tree_main_loop(agent_state: AgentState, client: Arc<RpmOstreeClient>) {
    let mut update_interval = interval(Duration::from_secs(
        agent_state.config.poll_interval_secs.into(),
    ));

    loop {
        update_interval.tick().await;

        let target_os_state = get_target_os_state().await;
        let host_os_state = match client.status().await {
            Ok(_status) => get_host_os_state().await,
            Err(e) => {
                error!("Failed to fetch live rpm-ostree status: {:?}", e);
                continue;
            }
        };

        {
            let mut current_state = agent_state.os_state.lock().await;
            *current_state = host_os_state.clone();
        }

        handle_os_tree(
            &client,
            agent_state.os_state.lock().await.clone(),
            target_os_state,
        )
        .await;
    }
}

async fn get_target_os_state() -> OsState {
    OsState {
        update_pending: false,
        booted_image: String::from("current_latest_new"),
        update_ostree_commit: Some(String::from("next_latest")),
    }
}

async fn get_host_os_state() -> OsState {
    OsState {
        update_pending: false,
        booted_image: String::from("current_latest"),
        update_ostree_commit: Option::None,
    }
}

async fn handle_os_tree(client: &RpmOstreeClient, current_state: OsState, target_state: OsState) {
    info!("Checking for OS update");

    if current_state.booted_image != target_state.booted_image {
        info!(
            "New deployment detected! Current: {} -> Target: {}",
            current_state.booted_image, target_state.booted_image
        );

        match client.upgrade().await {
            Ok(()) => {
                info!("rpm-ostree upgrade staged successfully. Initiating system reboot...");

                if let Err(e) = client.apply_reboot().await {
                    error!(
                        "Critical: Upgrade succeeded but system reboot invocation failed: {:?}",
                        e
                    );
                }
            }
            Err(e) => {
                error!("OS upgrade failed execution: {:?}", e);
            }
        }
    } else {
        info!("System is up to date.");
    }
}
