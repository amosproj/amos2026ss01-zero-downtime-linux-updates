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

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, sea_query::prelude::serde_json};
    use serde_json::Value;

    #[test]
    fn app_assignment_update_doesnt_require_updated_at() {
        let update_json_str = r#"{ "application_config_id": 5, "device_id": 3 }"#;
        let update_json: Value = serde_json::from_str(update_json_str).unwrap();
        println!("Unmarshalled: {:?}", update_json);

        let app_ass_update = super::ActiveModel::from_json(update_json.clone());
        println!("Loaded ActiveModel: {:?}", app_ass_update);

        assert!(app_ass_update.is_ok());
    }
}
