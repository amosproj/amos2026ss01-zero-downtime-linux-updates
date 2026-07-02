use std::sync::Arc;
use std::{collections::HashMap, time::Duration};

use amos_common::entities::{ApplicationLog, LogLevel};
use chrono::Utc;
use futures_util::stream::BoxStream;
use tokio::sync::mpsc;
use tokio_stream::{StreamExt as _, StreamMap};
use tracing::{debug, warn};

use crate::api_client::ApiClient;

use super::{LogChunk, LogStreamKind};

fn stream_to_level(stream: LogStreamKind) -> LogLevel {
    match stream {
        LogStreamKind::Stdout => LogLevel::Info,
        LogStreamKind::Stderr => LogLevel::Error,
    }
}

type AppLogStream = BoxStream<'static, anyhow::Result<LogChunk>>;

/// Commands sent to the central log registry task.
enum RegistryCommand {
    /// Start tailing a new application's logs. Replaces any
    /// existing stream registered under the same application_id
    Add {
        application_id: i32,
        name: String,
        stream: AppLogStream,
    },
    /// Stop tailing logs for application_id
    Remove { application_id: i32 },
}

#[derive(Clone)]
pub struct AppLogRegistry {
    tx: mpsc::UnboundedSender<RegistryCommand>,
}

impl AppLogRegistry {
    /// Start tailing stream under application_id
    pub fn add(&self, application_id: i32, name: String, stream: AppLogStream) {
        // Send failures mean the registry task has shut down
        let _ = self.tx.send(RegistryCommand::Add {
            application_id,
            name,
            stream,
        });
    }

    /// Stop tailing logs for application_id
    pub fn remove(&self, application_id: i32) {
        let _ = self.tx.send(RegistryCommand::Remove { application_id });
    }

    /// A registry with no backing task, for tests that need an
    /// `AppLogRegistry` handle but don't care about what's done with it.
    /// `add`/`remove` silently no-op since the receiver is dropped.
    #[cfg(test)]
    pub(crate) fn noop() -> Self {
        let (tx, _rx) = mpsc::unbounded_channel();
        Self { tx }
    }
}

pub fn spawn_app_log_registry(
    api_client: Arc<ApiClient>,
    log_flush_interval: Duration,
    log_max_batch: usize,
    log_max_buffer: usize,
) -> AppLogRegistry {
    let (tx, mut rx) = mpsc::unbounded_channel::<RegistryCommand>();

    tokio::spawn(async move {
        let mut streams: StreamMap<i32, AppLogStream> = StreamMap::new();
        let mut buffers: HashMap<i32, Vec<ApplicationLog::CreateEntry>> = HashMap::new();
        let mut names: HashMap<i32, String> = HashMap::new();
        let mut interval = tokio::time::interval(log_flush_interval);
        interval.tick().await;

        loop {
            tokio::select! {
                cmd = rx.recv() => {
                    match cmd {
                        Some(RegistryCommand::Add { application_id, name, stream }) => {
                            debug!(application_id, "Registering application log stream");
                            streams.insert(application_id, stream);
                            names.insert(application_id, name);
                        }
                        Some(RegistryCommand::Remove { application_id }) => {
                            debug!(application_id, "Unregistering application log stream");
                            streams.remove(&application_id);
                            names.remove(&application_id);
                            if let Some(mut buffer) = buffers.remove(&application_id) {
                                flush_application_logs(&mut buffer, application_id, &api_client, log_max_buffer).await;
                            }
                        }
                        None => {
                            // All AppLogRegistry handles dropped
                            for (application_id, mut buffer) in buffers.drain() {
                                flush_application_logs(&mut buffer, application_id, &api_client, log_max_buffer).await;
                            }
                            return;
                        }
                    }
                }
                Some((application_id, result)) = streams.next() => {
                    match result {
                        Ok(chunk) => {
                            let entry = ApplicationLog::CreateEntry {
                                time: chunk.time.or_else(|| Some(Utc::now())),
                                level: stream_to_level(chunk.stream),
                                message: chunk.message,
                                source: names.get(&application_id).cloned(),
                            };
                            let buffer = buffers.entry(application_id).or_default();
                            buffer.push(entry);
                            if buffer.len() >= log_max_batch {
                                let mut to_flush = std::mem::take(buffer);
                                flush_application_logs(&mut to_flush, application_id, &api_client, log_max_buffer).await;
                                *buffers.entry(application_id).or_default() = to_flush;
                            }
                        }
                        Err(e) => {
                            warn!(application_id, error = %e, "Error reading container log stream");
                        }
                    }
                }
                _ = interval.tick() => {
                    for (application_id, buffer) in buffers.iter_mut() {
                        if !buffer.is_empty() {
                            flush_application_logs(buffer, *application_id, &api_client, log_max_buffer).await;
                        }
                    }
                }
            }
        }
    });

    AppLogRegistry { tx }
}

async fn flush_application_logs(
    buffer: &mut Vec<ApplicationLog::CreateEntry>,
    application_id: i32,
    api_client: &ApiClient,
    max_buffer: usize,
) {
    if buffer.is_empty() {
        return;
    }
    match api_client
        .push_application_logs(application_id, buffer.clone())
        .await
    {
        Ok(()) => buffer.clear(),
        Err(err) => {
            warn!(
                application_id,
                error = %err,
                buffered = buffer.len(),
                "Failed to ship application logs; will retry on next flush",
            );
            if buffer.len() > max_buffer {
                let drop_count = buffer.len() - max_buffer;
                buffer.drain(..drop_count);
                warn!(
                    application_id,
                    dropped = drop_count,
                    "Dropped oldest application log entries due to buffer cap",
                );
            }
        }
    }
}
