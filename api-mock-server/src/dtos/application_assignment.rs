use sea_orm::ActiveValue;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::{ApplicationConfig, Device, Group};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "application_assignments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub application_config_id: i32,
    #[sea_orm(belongs_to, from = "application_config_id", to = "id")]
    pub application_config: HasOne<ApplicationConfig::Entity>,

    pub device_id: Option<i32>,
    #[sea_orm(belongs_to, from = "device_id", to = "id")]
    pub device: HasOne<Device::Entity>,

    pub group_id: Option<i32>,
    #[sea_orm(belongs_to, from = "group_id", to = "id")]
    pub group: HasOne<Group::Entity>,

    pub deleted_at: Option<DateTimeUtc>,

    pub superseded_by: Option<i32>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let has_device = matches!(self.device_id.clone(), ActiveValue::Set(Some(_)));
        let has_group = matches!(self.group_id.clone(), ActiveValue::Set(Some(_)));

        if !has_device && !has_group {
            return Err(DbErr::Custom(
                "Either device_id or group_id must be set".into(),
            ));
        }

        Ok(self)
    }
}

impl Model {
    pub fn into_api(self) -> amos_common::entities::ApplicationAssignment::Model {
        amos_common::entities::ApplicationAssignment::Model {
            id: self.id,
            application_config_id: self.application_config_id,
            device_id: self.device_id,
            group_id: self.group_id,
            deleted_at: self.deleted_at,
            superseded_by: self.superseded_by,
        }
    }
}
