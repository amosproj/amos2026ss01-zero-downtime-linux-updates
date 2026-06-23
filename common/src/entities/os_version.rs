use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub commit_hash: String,
    pub orchestrator_version: String,
    pub description: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub superseded_by: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub commit_hash: String,
    pub orchestrator_version: String,
    pub description: Option<String>,
}
