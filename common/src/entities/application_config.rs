use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub device_id: Option<i32>,
    pub group_id: Option<i32>,
    pub application_id: i32,
    pub image: String,
    pub version: i32,
    pub config: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub superseded_by: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub device_id: Option<i32>,
    pub group_id: Option<i32>,
    pub application_id: i32,
    pub image: String,
    pub config: Option<String>,
}
