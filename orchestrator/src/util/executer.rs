use anyhow::Context;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

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

        // Drain both pipes to EOF before reaping the child: racing the reads
        // against `child.wait()` lets the exit branch win while output is
        // still buffered, silently truncating the captured output.
        let mut stdout_done = false;
        let mut stderr_done = false;
        while !stdout_done || !stderr_done {
            tokio::select! {
                res = stdout_reader.next_line(), if !stdout_done => {
                    match res {
                        Ok(Some(line)) => {
                            tracing::trace!(target: "bootc_subproc", "{}", line);
                            captured_stdout.push_str(&line);
                            captured_stdout.push('\n');
                        }
                        _ => stdout_done = true,
                    }
                },
                res = stderr_reader.next_line(), if !stderr_done => {
                    match res {
                        Ok(Some(line)) => {
                            tracing::trace!(target: "bootc_subproc_err", "{}", line);
                            captured_stderr.push_str(&line);
                            captured_stderr.push('\n');
                        }
                        _ => stderr_done = true,
                    }
                },
            }
        }

        let exit_status = child.wait().await?;
        let exit_code = if exit_status.code().is_none() {
            Some(137)
        } else {
            exit_status.code()
        };

        Ok(ExecResult {
            stdout: captured_stdout,
            stderr: captured_stderr,
            exit_code,
        })
    }
}
