use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Application, Device};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "device_application_configs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub device_id: i32,
    #[sea_orm(belongs_to, from = "device_id", to = "id")]
    pub device: HasOne<Device::Entity>,

    pub application_id: i32,
    #[sea_orm(belongs_to, from = "application_id", to = "id")]
    pub application: HasOne<Application::Entity>,

    pub config: String,

    pub version: i32,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::DeviceApplicationConfig::Model {
        amos_common::entities::DeviceApplicationConfig::Model {
            id: self.id,
            device_id: self.device_id,
            application_id: self.application_id,
            config: self.config,
            version: self.version,
        }
    }
}
