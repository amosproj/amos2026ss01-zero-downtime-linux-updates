use amos_common::entities::ApplicationConfig;

impl super::DataStore {
    pub async fn apps_get_assigned(
        &self,
        device_id: i32,
    ) -> Result<Vec<ApplicationConfig::Model>, sea_orm::DbErr> {
        crate::api_v1::db::list_application_configs_for_device(device_id).await
    }

    pub async fn apps_put_report(
        &self,
        device_id: i32,
        application_config_ids: impl Iterator<Item = i32>,
    ) -> Result<(), sea_orm::DbErr> {
        for config_id in application_config_ids {
            crate::api_v1::db::add_reported_application_assignment(config_id, device_id).await?;
        }

        Ok(())
    }
}
