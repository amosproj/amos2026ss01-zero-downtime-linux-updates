use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::ApplicationConfig;

#[derive(Clone, Debug, Deserialize)]
pub struct CreateApplication {
    pub name: String,
    pub description: String,
}

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "applications")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub name: String,

    pub description: String,

    #[sea_orm(has_many)]
    pub applications: HasMany<ApplicationConfig::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
