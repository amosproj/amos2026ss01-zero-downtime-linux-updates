use sea_orm::entity::prelude::*;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "os_versions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub commit_hash: String,

    pub orchestrator_version: String,

    pub description: Option<String>,
}

impl ActiveModelBehavior for ActiveModel {}
