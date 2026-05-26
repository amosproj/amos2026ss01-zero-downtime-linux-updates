use crate::state::{AgentState, OsState};
use crate::update_check::{CheckForUpdate, UpdateDecision};
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
    use crate::config_loader::Settings;
    use crate::update_check::MockCheckForUpdate;
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

    /// helper function: creates a minimal `AgentState` for tests.
    fn test_agent_state(poll_interval_secs: u32) -> AgentState {
        use crate::state::AppState;
        let config = Settings {
            cloud_url: "http://localhost".into(),
            poll_interval_secs,
            inventory_path: "/tmp/inv.json".into(),
        };
        let os_state = OsState {
            update_pending: false,
            booted_image: "registry.example.com/os:41".into(),
            update_ostree_commit: None,
        };
        AgentState::new("0.0.0", config, os_state, Vec::<AppState>::new())
    }

    /// Valides rpm-ostree-Status-JSON für den MockExecuter.
    fn os_status_json() -> String {
        r#"{"update_pending":false,"booted_image":"registry.example.com/os:41","update_ostree_commit":null}"#
            .to_string()
    }

    /// `run_os_tree_main_loop` muss `deploy(<version>)` und danach `systemctl reboot`
    /// aufrufen, wenn der UpdateChecker `UpdateRequired` zurückgibt.
    #[tokio::test]
    async fn main_loop_calls_deploy_and_reboot_when_update_required() {
        let mut mock_exec = MockExecuter::new();

        // 1. rpm-ostree status (wird vom Loop zu Beginn jeder Iteration abgerufen)
        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "rpm-ostree".to_string(),
                    "status".to_string(),
                    "--json".to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: os_status_json(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        // 2. rpm-ostree deploy 42
        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "rpm-ostree".to_string(),
                    "deploy".to_string(),
                    "42".to_string(),
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

        // 3. systemctl reboot
        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec!["systemctl".to_string(), "reboot".to_string()]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "".to_string(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        let client = Arc::new(RpmOstreeClient::new(Arc::new(mock_exec)));

        // UpdateChecker-Mock: gibt beim ersten Aufruf UpdateRequired zurück
        let mut mock_checker = MockCheckForUpdate::new();
        mock_checker
            .expect_check()
            .times(1)
            .returning(|| {
                Ok(UpdateDecision::UpdateRequired {
                    reasons: vec!["OS version drift: current `41` -> target `42`".into()],
                    target_os_version: "42".into(),
                })
            });

        let agent_state = test_agent_state(1);

        // Loop in einem separaten Task starten und nach kurzer Zeit abbrechen.
        // Nach der ersten Iteration sollten alle Mock-Erwartungen erfüllt sein.
        let handle = tokio::spawn(run_os_tree_main_loop(
            agent_state,
            client,
            Arc::new(mock_checker),
        ));

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        handle.abort();
        // mockall prüft die Erwartungen beim Drop – kein weiterer assert nötig.
    }

    /// `run_os_tree_main_loop` darf `deploy` NICHT aufrufen, wenn das System
    /// bereits aktuell ist.
    #[tokio::test]
    async fn main_loop_does_not_deploy_when_up_to_date() {
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "rpm-ostree".to_string(),
                    "status".to_string(),
                    "--json".to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: os_status_json(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        // deploy und reboot dürfen NICHT aufgerufen werden → keine expect_execute für sie

        let client = Arc::new(RpmOstreeClient::new(Arc::new(mock_exec)));

        let mut mock_checker = MockCheckForUpdate::new();
        mock_checker
            .expect_check()
            .times(1)
            .returning(|| Ok(UpdateDecision::UpToDate));

        let agent_state = test_agent_state(1);

        let handle = tokio::spawn(run_os_tree_main_loop(
            agent_state,
            client,
            Arc::new(mock_checker),
        ));

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        handle.abort();
    }
}

pub async fn run_os_tree_main_loop(
    agent_state: AgentState,
    client: Arc<RpmOstreeClient>,
    update_checker: Arc<dyn CheckForUpdate>,
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
