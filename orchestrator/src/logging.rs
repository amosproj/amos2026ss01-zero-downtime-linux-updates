use std::sync::Arc;

use amos_common::entities::{DeviceLog, LogLevel};
use chrono::Utc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use crate::download_manager::DownloadManager;

const LOG_INTERNAL_TARGET: &str = "amos_orchestrator::log_internal";

struct MessageVisitor {
    message: Option<String>,
}

impl Visit for MessageVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message = Some(format!("{value:?}"));
        }
    }
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_owned());
        }
    }
}

fn tracing_level_to_log_level(level: &Level) -> LogLevel {
    match *level {
        Level::ERROR => LogLevel::Error,
        Level::WARN => LogLevel::Warn,
        Level::INFO => LogLevel::Info,
        Level::DEBUG => LogLevel::Debug,
        Level::TRACE => LogLevel::Trace,
    }
}

struct DeviceLogLayer {
    sender: UnboundedSender<DeviceLog::CreateEntry>,
}

impl<S> Layer<S> for DeviceLogLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        // Skip shipper diagnostics to prevent a feedback loop where API
        // failures generate logs that re-enter the channel and amplify.
        if metadata.target() == LOG_INTERNAL_TARGET {
            return;
        }
        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);
        let Some(message) = visitor.message else {
            return;
        };
        let entry = DeviceLog::CreateEntry {
            time: Some(Utc::now()),
            level: tracing_level_to_log_level(metadata.level()),
            message,
            source: metadata.module_path().map(str::to_owned),
        };
        let _ = self.sender.send(entry);
    }
}

/// Initialize the global tracing subscriber and capture layer.
///
/// Returns the receiver end of the log channel. Pass it to
/// `spawn_log_shipper` once the `DownloadManager` is ready.
///
/// log to journald *or* stdout, never both, to avoid duplicate journal
/// entries: under systemd, stdout is already captured into the journal
pub fn init(verbosity: u8) -> UnboundedReceiver<DeviceLog::CreateEntry> {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    // Restrict to our own crates by default; everything else stays at warn so
    // dependency logs don't pollute stdout or the DB stream. `RUST_LOG`
    // overrides the default when set.
    let default_filter = format!("amos_orchestrator={level},amos_common={level},warn");
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    // Try journald only when systemd has wired our stdout to the journal;
    // otherwise we want plain console output for interactive/dev runs.
    let journald_layer = if std::env::var_os("JOURNAL_STREAM").is_some() {
        tracing_journald::layer().ok()
    } else {
        None
    };
    // Use stdout when not under systemd, or as a fallback if connecting to the
    // journald socket failed
    let stdout_layer = if journald_layer.is_none() {
        Some(fmt::layer().with_target(true))
    } else {
        None
    };

    let (tx, rx) = mpsc::unbounded_channel::<DeviceLog::CreateEntry>();
    let device_log_layer = DeviceLogLayer { sender: tx };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(journald_layer)
        .with(device_log_layer)
        .init();

    rx
}

/// Spawn the background task that ships buffered device log entries to the API.
///
/// Call this after the `DownloadManager` is constructed. Events emitted
/// between `init` and this call are buffered in the channel and shipped on
/// the first flush.
pub fn spawn_log_shipper(
    mut rx: UnboundedReceiver<DeviceLog::CreateEntry>,
    download_manager: Arc<DownloadManager>,
) {
    let config = Arc::clone(&download_manager.config);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(
            config.log_flush_interval_secs,
        ));
        interval.tick().await;
        let mut buffer: Vec<DeviceLog::CreateEntry> = Vec::new();

        loop {
            tokio::select! {
                maybe_entry = rx.recv() => {
                    match maybe_entry {
                        Some(entry) => {
                            buffer.push(entry);
                            if buffer.len() >= config.log_max_batch {
                                flush(&mut buffer, &download_manager, config.log_max_buffer).await;
                            }
                        }
                        None => {
                            flush(&mut buffer, &download_manager, config.log_max_buffer).await;
                            return;
                        }
                    }
                }
                _ = interval.tick() => {
                    if !buffer.is_empty() {
                        flush(&mut buffer, &download_manager, config.log_max_buffer).await;
                    }
                }
            }
        }
    });
}

async fn flush(
    buffer: &mut Vec<DeviceLog::CreateEntry>,
    download_manager: &DownloadManager,
    max_buffer: usize,
) {
    if buffer.is_empty() {
        return;
    }
    match download_manager.push_device_logs(buffer.clone()).await {
        Ok(()) => {
            buffer.clear();
        }
        Err(err) => {
            tracing::warn!(
                target: LOG_INTERNAL_TARGET,
                error = %err,
                buffered = buffer.len(),
                "Failed to ship device logs; will retry on next flush",
            );
            // Drop oldest entries when the buffer exceeds the cap to bound
            // memory usage during a sustained cloud outage.
            if buffer.len() > max_buffer {
                let drop_count = buffer.len() - max_buffer;
                buffer.drain(..drop_count);
                tracing::warn!(
                    target: LOG_INTERNAL_TARGET,
                    dropped = drop_count,
                    "Dropped oldest device log entries due to buffer cap",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_log_layer_captures_event_fields() {
        let (tx, mut rx) = mpsc::unbounded_channel::<DeviceLog::CreateEntry>();
        let layer = DeviceLogLayer { sender: tx };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello from test");
        });
        let entry = rx.try_recv().expect("expected one entry");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "hello from test");
        let source = entry.source.expect("module_path should be set");
        assert!(
            source.starts_with("amos_orchestrator"),
            "unexpected source: {source}",
        );
    }

    #[test]
    fn device_log_layer_skips_its_own_target() {
        let (tx, mut rx) = mpsc::unbounded_channel::<DeviceLog::CreateEntry>();
        let layer = DeviceLogLayer { sender: tx };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: LOG_INTERNAL_TARGET, "heartbeat");
        });
        assert!(rx.try_recv().is_err());
    }
}
