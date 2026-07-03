use anyhow::Context;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[async_trait]
#[cfg_attr(test, mockall::automock)]
pub trait Executer: Send + Sync {
    async fn execute(&self, command: String, args: Vec<String>) -> anyhow::Result<ExecResult>;
}

pub struct RealExecuter;

#[async_trait]
impl Executer for RealExecuter {
    async fn execute(&self, command: String, args: Vec<String>) -> anyhow::Result<ExecResult> {
        let mut child = Command::new(&command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdout = child.stdout.take().context("Failed to open stdout")?;
        let stderr = child.stderr.take().context("Failed to open stderr")?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        let mut captured_stdout = String::new();
        let mut captured_stderr = String::new();

        loop {
            tokio::select! {
                res = stdout_reader.next_line() => {
                    if let Ok(Some(line)) = res {
                        info!(target: "bootc_subproc", "{}", line);
                        captured_stdout.push_str(&line);
                        captured_stdout.push('\n');
                    }
                },
                res = stderr_reader.next_line() => {
                    if let Ok(Some(line)) = res {
                        info!(target: "bootc_subproc_err", "{}", line);
                        captured_stderr.push_str(&line);
                        captured_stderr.push('\n');
                    }
                },
                status = child.wait() => {
                    let exit_status = status?;

                    let exit_code = if exit_status.code().is_none() {
                        Some(137)
                    } else {
                        exit_status.code()
                    };

                    return Ok(ExecResult {
                        stdout: captured_stdout,
                        stderr: captured_stderr,
                        exit_code,
                    });
                }
            }
        }
    }
}