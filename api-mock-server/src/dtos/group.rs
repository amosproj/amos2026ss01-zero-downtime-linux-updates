use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::Device;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub name: String,

    pub deleted_at: Option<DateTimeUtc>,

    pub superseded_by: Option<i32>,

    #[sea_orm(has_many)]
    pub devices: HasMany<Device::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::Group::Model {
        amos_common::entities::Group::Model {
            id: self.id,
            name: self.name,
            deleted_at: self.deleted_at,
            superseded_by: self.superseded_by,
        }
    }
}
