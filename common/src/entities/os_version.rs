use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
pub struct CreateOsVersion {
    pub commit_hash: String,
    pub orchestrator_version: String,
    pub description: Option<String>,
}

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "os_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub commit_hash: String,

    pub orchestrator_version: String,

    pub description: Option<String>,
}

impl ActiveModelBehavior for ActiveModel {}
