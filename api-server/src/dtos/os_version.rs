use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "os_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub commit_hash: String,

    pub orchestrator_version: String,

    pub description: Option<String>,

    pub deleted_at: Option<DateTimeUtc>,

    pub superseded_by: Option<i32>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::OsVersion::Model {
        amos_common::entities::OsVersion::Model {
            id: self.id,
            commit_hash: self.commit_hash,
            orchestrator_version: self.orchestrator_version,
            description: self.description,
            deleted_at: self.deleted_at,
            superseded_by: self.superseded_by,
        }
    }
}
