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

    pub deleted_at: Option<DateTimeUtc>,

    pub superseded_by: Option<i32>,
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
    pub fn into_api(self) -> amos_common::entities::ReportedApplicationAssignment::Model {
        amos_common::entities::ReportedApplicationAssignment::Model {
            id: self.id,
            application_config_id: self.application_config_id,
            device_id: self.device_id,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
            superseded_by: self.superseded_by,
        }
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};

    #[test]
    fn app_assignment_update_doesnt_require_updated_at() {
        let am = super::ActiveModel {
            id: Set(1),
            application_config_id: Set(5),
            device_id: Set(3),
            updated_at: sea_orm::ActiveValue::NotSet,
            deleted_at: sea_orm::ActiveValue::NotSet,
            superseded_by: sea_orm::ActiveValue::NotSet,
        };
        assert!(am.is_not_set(<super::Entity as sea_orm::EntityTrait>::Column::UpdatedAt));
    }
}
