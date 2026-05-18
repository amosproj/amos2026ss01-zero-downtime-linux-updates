use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait Executer: Send + Sync {
    async fn execute(&self, command: String, args: Vec<String>) -> anyhow::Result<ExecResult>;
}

pub struct RealExecuter;

#[async_trait]
impl Executer for RealExecuter {
    async fn execute(&self, command: String, args: Vec<String>) -> anyhow::Result<ExecResult> {
        let output = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        // Handle 137 signal (SIGKILL + 128) logic if killed by an atomic reboot context
        let exit_code = if output.status.code().is_none() {
            Some(137)
        } else {
            output.status.code()
        };

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code,
        })
    }
}
