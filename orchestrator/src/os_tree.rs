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

    #[allow(dead_code)]
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

    /// Deploy a specific OS version as defined by the cloud database.
    /// Runs `rpm-ostree deploy <version>` so that the requested version is
    /// installed instead of blindly pulling the latest available.
    pub async fn deploy(&self, version: &str) -> Result<()> {
        let res = self
            .executer
            .execute(
                "sudo".to_string(),
                vec![
                    "rpm-ostree".to_string(),
                    "deploy".to_string(),
                    version.to_string(),
                ],
            )
            .await?;

        if res.exit_code != Some(0) {
            return Err(anyhow!(
                "rpm-ostree deploy {} failed: {}",
                version,
                res.stderr
            ));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::executer::{ExecResult, MockExecuter};
    use mockall::predicate::eq;

    #[tokio::test]
    async fn deploy_calls_rpm_ostree_deploy_with_correct_version() {
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "rpm-ostree".to_string(),
                    "deploy".to_string(),
                    "41".to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "Staging deployment...done".to_string(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        let client = RpmOstreeClient::new(Arc::new(mock_exec));
        let result = client.deploy("41").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn deploy_returns_error_on_nonzero_exit_code() {
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "rpm-ostree".to_string(),
                    "deploy".to_string(),
                    "99".to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "".to_string(),
                    stderr: "error: Version 99 not found in history".to_string(),
                    exit_code: Some(1),
                })
            });

        let client = RpmOstreeClient::new(Arc::new(mock_exec));
        let result = client.deploy("99").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("rpm-ostree deploy 99 failed"));
        assert!(err.contains("Version 99 not found in history"));
    }

    #[tokio::test]
    async fn deploy_uses_version_from_update_decision() {
        // Stellt sicher, dass die Version aus UpdateDecision 1:1 an rpm-ostree weitergereicht wird.
        let target_version = "42";
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "rpm-ostree".to_string(),
                    "deploy".to_string(),
                    target_version.to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "Deploying 42...done".to_string(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        let client = RpmOstreeClient::new(Arc::new(mock_exec));

        // Simuliert den Aufruf aus run_os_tree_main_loop:
        // UpdateDecision::UpdateRequired { target_os_version: "42", ... }
        let decision_version = "42".to_string();
        let result = client.deploy(&decision_version).await;
        assert!(result.is_ok());
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
            Ok(UpdateDecision::UpdateRequired {
                reasons,
                target_os_version,
            }) => {
                info!(
                    "Update required ({} reason(s)): {}",
                    reasons.len(),
                    reasons.join("; ")
                );
                info!(
                    "Deploying target OS version `{}` as defined by the database",
                    target_os_version
                );

                match client.deploy(&target_os_version).await {
                    Ok(()) => {
                        info!(
                            "rpm-ostree deploy `{}` staged successfully. Initiating system reboot...",
                            target_os_version
                        );

                        if let Err(e) = client.apply_reboot().await {
                            error!(
                                "Critical: Deploy succeeded but system reboot invocation failed: {:?}",
                                e
                            );
                        }
                    }
                    Err(e) => {
                        error!("OS deploy failed execution: {:?}", e);
                    }
                }
            }
            Err(e) => {
                warn!("Update check failed, will retry next tick: {:?}", e);
            }
        }
    }
}
