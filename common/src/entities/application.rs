use sea_orm::entity::prelude::*;

use super::ApplicationConfig;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
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
