use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::LogLevel;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: Uuid,
    pub time: DateTime<Utc>,
    pub device_id: i32,
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateEntry {
    pub time: Option<DateTime<Utc>>,
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub entries: Vec<CreateEntry>,
}
