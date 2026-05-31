use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: i32,
    pub application_id: i32,
    pub image: String,
    pub config: Option<String>,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateModel {
    pub application_id: i32,
    pub image: String,
    pub config: Option<String>,
    pub comment: Option<String>,
}
