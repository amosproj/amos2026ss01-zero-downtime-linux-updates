use crate::state::{AgentState, OsState};
use crate::update_check::{UpdateChecker, UpdateDecision};
use anyhow::{Context, Result, anyhow};

use log::{error, info, warn};
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

pub async fn run_os_tree_main_loop(
    agent_state: AgentState,
    client: Arc<RpmOstreeClient>,
    update_checker: Arc<UpdateChecker>,
) {
    let mut update_interval = interval(Duration::from_secs(
        agent_state.config.poll_interval_secs.into(),
    ));

    loop {
        update_interval.tick().await;

        let host_status = match client.status().await {
            Ok(status) => status,
            Err(e) => {
                error!("Failed to fetch live rpm-ostree status: {:?}", e);
                continue;
            }
        };

        {
            let mut current_state = agent_state.os_state.lock().await;
            *current_state = host_status;
        }

        info!("Checking for OS update");
        match update_checker.check().await {
            Ok(UpdateDecision::UpToDate) => {
                info!("System is up to date.");
            }
            Ok(UpdateDecision::UpdateRequired { reasons }) => {
                info!(
                    "Update required ({} reason(s)): {}",
                    reasons.len(),
                    reasons.join("; ")
                );

                match client.upgrade().await {
                    Ok(()) => {
                        info!(
                            "rpm-ostree upgrade staged successfully. Initiating system reboot..."
                        );

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
            }
            Err(e) => {
                warn!("Update check failed, will retry next tick: {:?}", e);
            }
        }
    }
}
