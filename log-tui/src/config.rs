use amos_common::entities::LogLevel;
use clap::Parser;

/// Command-line configuration for the log viewer.
///
/// The viewer is a *user*-API client: it only needs the user base URL and a
/// bearer JWT (the long-lived dev token used across the demo).
#[derive(Parser, Debug)]
#[command(
    name = "amos-log-tui",
    about = "Live log viewer for the AMOS device-management API"
)]
pub struct Cli {
    /// Base URL of the user API, including the `/v1` prefix.
    #[arg(long, env = "AMOS_BASE_URL", default_value = "http://localhost:8080/v1")]
    pub base_url: String,

    /// Bearer JWT for the user API (the long-lived dev token for the demo).
    #[arg(long, env = "AMOS_JWT", default_value = "")]
    pub jwt: String,

    /// Pre-select a device by id on startup.
    #[arg(long)]
    pub device: Option<i32>,

    /// Initial minimum-severity filter.
    #[arg(long, value_enum, default_value_t = LevelArg::Info)]
    pub level: LevelArg,

    /// Maximum number of log lines kept in the ring buffer.
    #[arg(long, default_value_t = 2000)]
    pub max_logs: usize,
}

/// CLI-friendly log level, including `all` (no minimum filter).
#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum LevelArg {
    All,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LevelArg {
    /// `None` = no minimum-severity filter (stream everything).
    pub fn to_level(self) -> Option<LogLevel> {
        match self {
            LevelArg::All => None,
            LevelArg::Trace => Some(LogLevel::Trace),
            LevelArg::Debug => Some(LogLevel::Debug),
            LevelArg::Info => Some(LogLevel::Info),
            LevelArg::Warn => Some(LogLevel::Warn),
            LevelArg::Error => Some(LogLevel::Error),
            LevelArg::Fatal => Some(LogLevel::Fatal),
        }
    }
}
