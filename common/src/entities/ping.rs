use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::Device;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "pings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub device_id: i32,
    #[sea_orm(belongs_to, from = "device_id", to = "id")]
    pub device: HasOne<Device::Entity>,

    pub reported_at: DateTimeUtc,
}

impl ActiveModelBehavior for ActiveModel {}
