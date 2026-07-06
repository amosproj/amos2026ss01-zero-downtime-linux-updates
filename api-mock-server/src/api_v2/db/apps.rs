use amos_common::entities::ApplicationConfig;

impl super::DataStore {
    pub async fn apps_get_assigned(
        &self,
        device_id: i32,
    ) -> Result<Vec<ApplicationConfig::Model>, sea_orm::DbErr> {
        crate::api_v1::db::list_application_configs_for_device(device_id).await
    }
}
