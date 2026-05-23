use sea_orm::ActiveValue::Set;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::prelude::chrono;
use serde::{Deserialize, Serialize};

use super::{ApplicationConfig, Device};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "reported_application_assignments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub application_config_id: i32,
    #[sea_orm(belongs_to, from = "application_config_id", to = "id")]
    pub application_config: HasOne<ApplicationConfig::Entity>,

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
