use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

/// Initialize the global tracing subscriber with two sinks: stdout and journald
/// (the journald sink is silently skipped when not running under systemd).
pub fn init(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    // Restrict to our own crates by default so dependency logs don't pollute
    // stdout or the journal. `RUST_LOG` overrides the default when set.
    let default_filter = format!("amos_orchestrator={level},amos_common={level},warn");
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    let stdout_layer = fmt::layer().with_target(true);
    let journald_layer = tracing_journald::layer().ok();

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(journald_layer)
        .init();
}
