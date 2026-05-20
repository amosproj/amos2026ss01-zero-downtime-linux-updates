use sea_orm::entity::prelude::*;

use super::{ApplicationConfig, Device, Group};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "application_assignments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub application_id: i32,
    #[sea_orm(belongs_to, from = "application_id", to = "id")]
    pub application: HasOne<ApplicationConfig::Entity>,

    pub device_id: Option<i32>,
    #[sea_orm(belongs_to, from = "device_id", to = "id")]
    pub device: HasOne<Device::Entity>,

    pub group_id: Option<i32>,
    #[sea_orm(belongs_to, from = "group_id", to = "id")]
    pub group: HasOne<Group::Entity>,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(
        self,
        _db: &C,
        _insert: bool,
    ) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let has_device = self.device_id.clone().into_value().is_some();
        let has_group = self.group_id.clone().into_value().is_some();

        if !has_device && !has_group {
            return Err(DbErr::Custom("Either device_id or group_id must be set".into()));
        }

        Ok(self)
    }
}
