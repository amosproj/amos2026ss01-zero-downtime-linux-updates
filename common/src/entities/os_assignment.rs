use sea_orm::entity::prelude::*;

use super::{Device, Group, OsVersion};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "os_assignments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub os_version_id: i32,
    #[sea_orm(belongs_to, from = "os_version_id", to = "id")]
    pub os_version: HasOne<OsVersion::Entity>,

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
