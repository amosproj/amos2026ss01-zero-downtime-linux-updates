use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::Device;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "tenants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub name: String,

    pub description: Option<String>,

    #[sea_orm(has_many)]
    pub devices: HasMany<Device::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}
