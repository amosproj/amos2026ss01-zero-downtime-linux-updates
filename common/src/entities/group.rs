use sea_orm::entity::prelude::*;

use super::Device;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub name: String,

    #[sea_orm(has_many)]
    pub devices: HasMany<Device::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
