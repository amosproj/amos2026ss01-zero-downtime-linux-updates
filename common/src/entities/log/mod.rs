pub mod application_log;
pub use self::application_log as ApplicationLog;

pub mod device_log;
pub use self::device_log as DeviceLog;

pub mod log_level;
pub use self::log_level::LogLevel;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogKind {
    Device,
    Application,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LogEvent {
    Device(DeviceLog::Model),
    Application(ApplicationLog::Model),
}

impl LogEvent {
    pub fn device_id(&self) -> i32 {
        match self {
            LogEvent::Device(model) => model.device_id,
            LogEvent::Application(model) => model.device_id,
        }
    }

    pub fn application_id(&self) -> Option<i32> {
        match self {
            LogEvent::Device(_) => None,
            LogEvent::Application(model) => Some(model.application_id),
        }
    }

    pub fn level(&self) -> LogLevel {
        match self {
            LogEvent::Device(model) => model.level,
            LogEvent::Application(model) => model.level,
        }
    }
}

/// Query params for `GET /logs/stream`: `?device_id=&application_id=&level=&kind=`
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct LogStreamQuery {
    pub device_id: Option<i32>,
    pub application_id: Option<i32>,
    /// Minimum severity: events with a level lower than this are excluded.
    pub level: Option<LogLevel>,
    /// Restrict to a single log kind: `device` or `application`.
    pub kind: Option<LogKind>,
}

/// Query params for the historic log endpoints:
/// `?device_id=&application_id=&level=&from=&to=&page=&page_size=`
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct LogQuery {
    pub device_id: Option<i32>,
    pub application_id: Option<i32>,
    /// Minimum severity: entries with a level lower than this are excluded.
    pub level: Option<LogLevel>,
    /// Only include entries at or after this time (inclusive).
    pub from: Option<DateTime<Utc>>,
    /// Only include entries at or before this time (inclusive).
    pub to: Option<DateTime<Utc>>,
}
