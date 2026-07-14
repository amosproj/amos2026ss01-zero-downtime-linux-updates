//! Sends logs to different sinks, including our own API

use std::sync::Arc;
use std::time::Duration;

use amos_common::entities::LogLevel;
use chrono::Utc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

use crate::api_client::ApiClient;

const LOG_INTERNAL_TARGET: &str = "amos_orchestrator::log_internal";

/// Message flowing through the log channel: either a captured tracing event,
/// or a request to flush whatever is buffered right now. Both share one
/// channel so a flush request is guaranteed to be processed after every
/// entry sent ahead of it (FIFO), instead of racing it via a separate channel.
pub enum LogMessage {
    Entry(amos_common::device_api::logs::PostBodyItem),
    Flush(oneshot::Sender<()>),
}

/// Handle for requesting an out-of-band flush of buffered device logs.
///
/// Used on fatal-error exit paths to give the last log entries (e.g. the
/// error that's about to kill the process) a chance to reach the cloud before
/// `std::process::exit` tears down the process, instead of waiting for the
/// shipper's periodic flush.
#[derive(Clone)]
pub struct LogFlusher {
    sender: UnboundedSender<LogMessage>,
}

impl LogFlusher {
    /// Flushes the buffer and waits for it to complete. A no-op if the
    /// shipper task isn't running (e.g. it hasn't been spawned yet).
    pub async fn flush(&self) {
        let (tx, rx) = oneshot::channel();
        if self.sender.send(LogMessage::Flush(tx)).is_ok() {
            let _ = rx.await;
        }
    }
}

pub struct OrchestratorLogger {
    log_rx: UnboundedReceiver<LogMessage>,
    buffer: Vec<amos_common::device_api::logs::PostBodyItem>,
}

impl OrchestratorLogger {
    /// Initialize the global tracing subscriber and capture layer.
    ///
    /// Returns the logger (pass it to `into_spawned_shipper` once the
    /// `ApiClient` is ready) and a `LogFlusher` for requesting an immediate
    /// flush on fatal-error paths.
    ///
    /// log to journald *or* stdout, never both, to avoid duplicate journal
    /// entries: under systemd, stdout is already captured into the journal
    pub fn init(verbosity: u8) -> (Self, LogFlusher) {
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

        let (tx, rx) = mpsc::unbounded_channel::<LogMessage>();
        let device_log_layer = DeviceLogLayer { sender: tx.clone() };

        tracing_subscriber::registry()
            .with(env_filter)
            .with(stdout_layer)
            .with(journald_layer)
            .with(device_log_layer)
            .init();

        (
            Self {
                log_rx: rx,
                buffer: Vec::new(),
            },
            LogFlusher { sender: tx },
        )
    }

    /// Spawn the background task that ships buffered device log entries to the API.
    ///
    /// Call this after the `ApiClient` is constructed. Events emitted
    /// between `init` and this call are buffered in the channel and shipped on
    /// the first flush.
    pub fn into_spawned_shipper(
        mut self,
        api_client: Arc<ApiClient>,
        log_flush_interval: Duration,
        log_max_batch: usize,
        log_max_buffer: usize,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(log_flush_interval);
            interval.tick().await;

            loop {
                tokio::select! {
                    maybe_msg = self.log_rx.recv() => {
                        match maybe_msg {
                            Some(LogMessage::Entry(entry)) => {
                                self.buffer.push(entry);
                                if self.buffer.len() >= log_max_batch {
                                    self.flush(&api_client, log_max_buffer).await;
                                }
                            }
                            Some(LogMessage::Flush(ack)) => {
                                self.flush(&api_client, log_max_buffer).await;
                                let _ = ack.send(());
                            }
                            None => {
                                self.flush(&api_client, log_max_buffer).await;
                                return;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if !self.buffer.is_empty() {
                            self.flush(&api_client, log_max_buffer).await;
                        }
                    }
                }
            }
        })
    }

    async fn flush(&mut self, api_client: &ApiClient, max_buffer: usize) {
        if self.buffer.is_empty() {
            return;
        }
        match api_client.push_device_logs(&self.buffer).await {
            Ok(()) => {
                self.buffer.clear();
            }
            Err(err) => {
                tracing::warn!(
                    target: LOG_INTERNAL_TARGET,
                    error = %err,
                    buffered = self.buffer.len(),
                    "Failed to ship device logs; will retry on next flush",
                );
                // Drop oldest entries when the buffer exceeds the cap to bound
                // memory usage during a sustained cloud outage.
                if self.buffer.len() > max_buffer {
                    let drop_count = self.buffer.len() - max_buffer;
                    self.buffer.drain(..drop_count);
                    tracing::warn!(
                        target: LOG_INTERNAL_TARGET,
                        dropped = drop_count,
                        "Dropped oldest device log entries due to buffer cap",
                    );
                }
            }
        }
    }
}

impl Drop for OrchestratorLogger {
    // Flushing here would require an async, fallible HTTP call inside a sync
    // destructor, and fatal exits go through `std::process::exit` which skips
    // destructors entirely anyway. Fatal paths use `LogFlusher::flush` instead
    // (see module docs above).
    fn drop(&mut self) {}
}

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
    sender: UnboundedSender<LogMessage>,
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
        let entry = amos_common::device_api::logs::PostBodyItem {
            time: Some(Utc::now()),
            level: tracing_level_to_log_level(metadata.level()),
            message,
            source: metadata.module_path().map(str::to_owned),
        };
        let _ = self.sender.send(LogMessage::Entry(entry));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_log_layer_captures_event_fields() {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogMessage>();
        let layer = DeviceLogLayer { sender: tx };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello from test");
        });
        let LogMessage::Entry(entry) = rx.try_recv().expect("expected one entry") else {
            panic!("expected a LogMessage::Entry");
        };
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
        let (tx, mut rx) = mpsc::unbounded_channel::<LogMessage>();
        let layer = DeviceLogLayer { sender: tx };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: LOG_INTERNAL_TARGET, "heartbeat");
        });
        assert!(rx.try_recv().is_err());
    }
}
