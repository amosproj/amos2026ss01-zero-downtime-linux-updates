use chrono::{DateTime, Utc};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, fmt};

const DB_STUB_TARGET: &str = "amos_orchestrator::db_stub";

/// Shaped to match the `POST /v1/logs/devices` request body in
/// `Documentation/log_api.md`. a future HTTP shipper may serializes this directly.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LogEntry {
    pub time: DateTime<Utc>,
    pub level: &'static str,
    pub message: String,
    pub source: Option<String>,
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

fn level_str(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "error",
        Level::WARN => "warn",
        Level::INFO => "info",
        Level::DEBUG => "debug",
        Level::TRACE => "trace",
    }
}

struct DbStubLayer {
    sender: UnboundedSender<LogEntry>,
}

impl<S> Layer<S> for DbStubLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let metadata = event.metadata();
        // The stub itself uses tracing for its periodic summary; skip those
        // events to avoid feeding our own output back into the channel.
        if metadata.target() == DB_STUB_TARGET {
            return;
        }
        let mut visitor = MessageVisitor { message: None };
        event.record(&mut visitor);
        let Some(message) = visitor.message else {
            return;
        };
        let entry = LogEntry {
            time: Utc::now(),
            level: level_str(metadata.level()),
            message,
            source: metadata.module_path().map(str::to_owned),
        };
        let _ = self.sender.send(entry);
    }
}

/// Initialize the global tracing subscriber with a console/journal sink plus an
/// in-memory stub for the future DB shipper. Must be called from inside a Tokio
/// runtime so the stub consumer task can be spawned.
///
/// log to journald *or* stdout, never both, to avoid duplicate journal
/// entries: under systemd, stdout is already captured into the journal
pub fn init(verbosity: u8, device_uuid: &str) {
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

    let (tx, rx) = mpsc::unbounded_channel::<LogEntry>();
    let db_stub_layer = DbStubLayer { sender: tx };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(journald_layer)
        .with(db_stub_layer)
        .init();

    spawn_db_stub_consumer(rx, device_uuid.to_owned());
}

fn spawn_db_stub_consumer(mut rx: UnboundedReceiver<LogEntry>, device_uuid: String) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.tick().await;
        let mut buffered: u64 = 0;
        loop {
            tokio::select! {
                maybe_entry = rx.recv() => {
                    match maybe_entry {
                        Some(_entry) => buffered += 1,
                        None => return,
                    }
                }
                _ = interval.tick() => {
                    if buffered > 0 {
                        tracing::info!(
                            target: DB_STUB_TARGET,
                            device_uuid = %device_uuid,
                            buffered,
                            "db log stub: would have shipped entries to POST /v1/logs/devices",
                        );
                        buffered = 0;
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_stub_layer_captures_event_fields() {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogEntry>();
        let layer = DbStubLayer { sender: tx };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("hello from test");
        });
        let entry = rx.try_recv().expect("expected one entry");
        assert_eq!(entry.level, "info");
        assert_eq!(entry.message, "hello from test");
        let source = entry.source.expect("module_path should be set");
        assert!(
            source.starts_with("amos_orchestrator"),
            "unexpected source: {source}",
        );
    }

    #[test]
    fn db_stub_layer_skips_its_own_target() {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogEntry>();
        let layer = DbStubLayer { sender: tx };
        let subscriber = tracing_subscriber::registry().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: DB_STUB_TARGET, "heartbeat");
        });
        assert!(rx.try_recv().is_err());
    }
}
