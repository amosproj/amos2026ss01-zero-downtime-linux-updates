use serde::{Deserialize, Serialize};

fn default_version() -> i32 {
    1
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub device_id: Option<i32>,
    pub group_id: Option<i32>,
    pub application_id: i32,
    pub image: String,
    pub config: String,
    pub version: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub device_id: Option<i32>,
    pub group_id: Option<i32>,
    pub application_id: i32,
    pub image: String,
    pub config: String,
    #[serde(default = "default_version")]
    pub version: i32,
}
