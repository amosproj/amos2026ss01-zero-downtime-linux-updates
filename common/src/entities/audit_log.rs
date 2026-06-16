use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    pub id: i64,
    pub table_name: String,
    pub record_id: String,
    pub operation: String,
    pub old_data: Option<serde_json::Value>,
    pub new_data: Option<serde_json::Value>,
    pub changed_by: i32,
    pub changed_at: DateTime<Utc>,
}
