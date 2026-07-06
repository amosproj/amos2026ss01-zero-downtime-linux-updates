impl super::DataStore {
    pub async fn ping_upsert(&self, device_id: i32) -> Result<(), sea_orm::DbErr> {
        crate::api_v1::db::upsert_ping(device_id).await
    }
}
