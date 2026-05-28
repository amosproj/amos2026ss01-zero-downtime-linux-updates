use crate::util::executer::*;
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

// Rudimentary representation of bootc status.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BootcStatus {
    pub booted: Option<BootcDeploymentInfo>,
    pub staged: Option<BootcDeploymentInfo>,
    pub rollback: Option<BootcDeploymentInfo>,
    pub rollback_queued: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(from = "BootcDeploymentInfoWire")]
pub struct BootcDeploymentInfo {
    pub checksum: String,
    pub image: Option<BootcImageInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BootcImageInfo {
    pub image_ref: String,
    pub transport: String,
    pub image_digest: Option<String>,
    pub version: Option<String>,
}

// Wire-transfer Deserialization Helpers for Serde
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootcStatusJsonWrapper {
    #[expect(dead_code)]
    pub api_version: String,
    #[expect(dead_code)]
    pub kind: String,
    #[expect(dead_code)]
    pub metadata: serde_json::Value,
    #[expect(dead_code)]
    pub spec: serde_json::Value,
    pub status: BootcStatus,
}

#[derive(Deserialize)]
struct BootcDeploymentInfoWire {
    ostree: OstreeWire,
    image: Option<BootcImageContainerWire>,
}

#[derive(Deserialize)]
struct OstreeWire {
    checksum: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootcImageContainerWire {
    image: BootcImageDetailsWire,
    image_digest: Option<String>,
    version: Option<String>,
}

#[derive(Deserialize)]
struct BootcImageDetailsWire {
    image: String,
    transport: String,
}

impl From<BootcDeploymentInfoWire> for BootcDeploymentInfo {
    fn from(wire: BootcDeploymentInfoWire) -> Self {
        let image = wire.image.map(|img| BootcImageInfo {
            image_ref: img.image.image,
            transport: img.image.transport,
            image_digest: img.image_digest,
            version: img.version,
        });
        BootcDeploymentInfo {
            checksum: wire.ostree.checksum,
            image,
        }
    }
}

// Client that does work.
pub struct Bootc {
    executer: Box<dyn Executer>,
    command_name: String,
}

impl Bootc {
    pub fn new(executer: Box<dyn Executer>) -> Self {
        Self {
            executer,
            command_name: "sudo".to_string(),
        }
    }

    // Helper fn
    #[allow(dead_code)]
    fn handle_exit_code(&self, code: Option<i32>) -> Result<()> {
        match code {
            Some(0) | Some(137) => Ok(()),
            _ => Err(anyhow!("Command failed with exit code: {:?}", code)),
        }
    }

    // Helper fn
    fn image_to_bootc_target(&self, image: &str) -> Result<String> {
        // format image string for bootc here
        Ok(image.to_string())
    }

    /// Helper to route all commands through 'sudo'
    async fn run_bootc_root(
        &self,
        sub_args: Vec<String>,
    ) -> Result<crate::util::executer::ExecResult> {
        let mut final_args = vec!["bootc".to_string()];
        final_args.extend(sub_args);

        self.executer
            .execute(self.command_name.clone(), final_args)
            .await
    }

    /// Returns the current bootc host status.
    #[instrument(skip(self), fields(prefix = "bootc"))]
    pub async fn status(&self) -> Result<BootcStatus> {
        let args = vec!["status".to_string(), "--json".to_string()];
        let res = self.run_bootc_root(args).await?;

        if res.exit_code != Some(0) {
            return Err(anyhow!("bootc status error: {}", res.stderr));
        }

        let wrapper: BootcStatusJsonWrapper =
            serde_json::from_str(&res.stdout).context("Failed to parse bootc status JSON")?;

        Ok(wrapper.status)
    }

    /// Pulls the image and stages it for the next boot.
    #[allow(dead_code)]
    pub async fn switch(&self, image: &str) -> Result<()> {
        let target = self.image_to_bootc_target(image)?;

        info!(?target, "Switching system image");

        let args = vec![
            "switch".to_string(),
            "--transport".to_string(),
            "containers-storage".to_string(),
            "--retain".to_string(),
            target,
        ];

        let res = self.run_bootc_root(args).await?;

        if res.exit_code == Some(0) {
            info!("Switching image complete");
            Ok(())
        } else {
            error!(exit_code = ?res.exit_code, "Switching image failed");
            Err(anyhow!(
                "Switching image failed with exit code: {:?}",
                res.exit_code
            ))
        }
    }

    #[allow(dead_code)]
    pub async fn rollback(&self) -> Result<()> {
        let args = vec!["rollback".to_string(), "--apply".to_string()];
        let res = self.run_bootc_root(args).await?;

        // Use helper to treat 137 as success
        self.handle_exit_code(res.exit_code)
    }

    #[allow(dead_code)]
    pub async fn apply(&self) -> Result<()> {
        let args = vec!["upgrade".to_string(), "--apply".to_string()];
        let res = self.run_bootc_root(args).await?;

        self.handle_exit_code(res.exit_code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::executer::{ExecResult, MockExecuter};
    use mockall::predicate::*;

    #[tokio::test]
    async fn test_bootc_status_success() {
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                mockall::predicate::eq("sudo".to_string()),
                mockall::predicate::eq(vec![
                    "bootc".to_string(),
                    "status".to_string(),
                    "--json".to_string()
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: r#"{
                        "status": {
                            "booted": {
                                "ostree": { "checksum": "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443" },
                                "image": null
                            },
                            "staged": null,
                            "rollback": null,
                            "rollbackQueued": false
                        }
                    }"#.to_string(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        let bootc_client = Bootc::new(Box::new(mock_exec));
        let result = bootc_client.status().await.unwrap();
        let booted_info = result
            .booted
            .as_ref()
            .expect("Expected 'booted' deployment details to be populated");

        assert_eq!(
            booted_info.checksum,
            "029b843f50ab1dd56ecc4d3eabb94f1aace5d958794ae4c2c72a915ee1b10443"
        );
        assert!(!result.rollback_queued);
    }

    #[tokio::test]
    async fn test_rollback_reboot_success() {
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "bootc".to_string(),
                    "rollback".to_string(),
                    "--apply".to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "".to_string(),
                    stderr: "".to_string(),
                    exit_code: Some(137),
                })
            });

        let client = Bootc::new(Box::new(mock_exec));
        assert!(client.rollback().await.is_ok());
    }

    #[tokio::test]
    async fn test_apply_hardware_failure() {
        let mut mock_exec = MockExecuter::new();

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "bootc".to_string(),
                    "upgrade".to_string(),
                    "--apply".to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "".to_string(),
                    stderr: "No space left on device".to_string(),
                    exit_code: Some(1),
                })
            });

        let client = Bootc::new(Box::new(mock_exec));
        let result = client.apply().await;

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("exit code: Some(1)")
        );
    }

    #[tokio::test]
    async fn test_switch_success() {
        let mut mock_exec = MockExecuter::new();
        let target_image = "quay.io/repo:latest";

        mock_exec
            .expect_execute()
            .with(
                eq("sudo".to_string()),
                eq(vec![
                    "bootc".to_string(),
                    "switch".to_string(),
                    "--transport".to_string(),
                    "containers-storage".to_string(),
                    "--retain".to_string(),
                    target_image.to_string(),
                ]),
            )
            .times(1)
            .returning(|_, _| {
                Ok(ExecResult {
                    stdout: "success".to_string(),
                    stderr: "".to_string(),
                    exit_code: Some(0),
                })
            });

        let client = Bootc::new(Box::new(mock_exec));
        let result = client.switch(target_image).await;

        assert!(result.is_ok());
    }
}
