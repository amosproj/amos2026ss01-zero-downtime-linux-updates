use sea_orm::ActiveValue::Set;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::prelude::chrono;
use serde::{Deserialize, Serialize};

use super::{Device, OsVersion};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reported_os_assignments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub os_version_id: i32,
    #[sea_orm(belongs_to, from = "os_version_id", to = "id")]
    pub os_version: HasOne<OsVersion::Entity>,

    pub device_id: i32,
    #[sea_orm(belongs_to, from = "device_id", to = "id")]
    pub device: HasOne<Device::Entity>,

    pub updated_at: DateTimeUtc,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        self.updated_at = Set(chrono::Utc::now());
        Ok(self)
    }
}

impl Model {
    pub fn into_api(self) -> amos_common::entities::ReportedOsAssignment::Model {
        amos_common::entities::ReportedOsAssignment::Model {
            id: self.id,
            os_version_id: self.os_version_id,
            device_id: self.device_id,
            updated_at: self.updated_at,
        }
    }
}
