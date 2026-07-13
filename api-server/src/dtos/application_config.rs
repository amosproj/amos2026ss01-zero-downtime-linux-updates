use amos_common::entities::ContainerConfigV1;

use sea_orm::ActiveValue;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Application, Device, Group};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "application_configs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub device_id: Option<i32>,
    #[sea_orm(belongs_to, from = "device_id", to = "id")]
    pub device: HasOne<Device::Entity>,

    pub group_id: Option<i32>,
    #[sea_orm(belongs_to, from = "group_id", to = "id")]
    pub group: HasOne<Group::Entity>,

    pub application_id: i32,
    #[sea_orm(belongs_to, from = "application_id", to = "id")]
    pub application: HasOne<Application::Entity>,

    pub image: String,

    pub config_version: i32,
    pub config: Option<String>,

    pub version: i32,

    pub deleted_at: Option<DateTimeUtc>,

    pub superseded_by: Option<i32>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if matches!(self.device_id, ActiveValue::Unchanged(_))
            && matches!(self.group_id, ActiveValue::Unchanged(_))
        {
            return Ok(self);
        }

        let has_device = matches!(self.device_id.clone(), ActiveValue::Set(Some(_)));
        let has_group = matches!(self.group_id.clone(), ActiveValue::Set(Some(_)));

        if !has_device && !has_group {
            return Err(DbErr::Custom(
                "Either device_id or group_id must be set".into(),
            ));
        }
        if has_device && has_group {
            return Err(DbErr::Custom(
                "Only one of device_id or group_id may be set, not both".into(),
            ));
        }

        Ok(self)
    }
}

impl Model {
    pub fn into_api(self) -> amos_common::entities::ApplicationConfig::Model {
        // TODO: Maybe return a HTTP error instead?
        let config: Option<ContainerConfigV1> =
            self.config
                .as_ref()
                .and_then(|json| match serde_json::from_str(json) {
                    Ok(c) => Some(c),
                    Err(e) => {
                        log::warn!("Invalid config JSON for app {}: {}", self.id, e);
                        None
                    }
                });

        amos_common::entities::ApplicationConfig::Model {
            id: self.id,
            device_id: self.device_id,
            group_id: self.group_id,
            application_id: self.application_id,
            image: self.image,
            config_version: self.config_version,
            config,
            version: self.version,
            deleted_at: self.deleted_at,
            superseded_by: self.superseded_by,
        }
    }
}
