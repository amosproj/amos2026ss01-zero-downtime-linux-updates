use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub uuid: String,
    pub public_key: Option<String>,
    pub hostname: String,
    pub tenant_id: i32,
    pub group_id: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub uuid: String,
    pub public_key: Option<String>,
    pub hostname: String,
    pub tenant_id: i32,
    pub group_id: Option<i32>,
}
