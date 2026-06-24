use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub os_version_id: i32,
    pub device_id: Option<i32>,
    pub group_id: Option<i32>,
    pub immediate: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub superseded_by: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub os_version_id: i32,
    pub device_id: Option<i32>,
    pub group_id: Option<i32>,
    pub immediate: Option<bool>,
}
