use axum::http::StatusCode;

use crate::auth::extractors::AuthDevice;

/// PUT /device/ping - Send aliveness signal
pub async fn put(AuthDevice(device): AuthDevice) -> StatusCode {
    match ping_upsert(device.id).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn ping_upsert(device_id: i32) -> Result<(), sea_orm::DbErr> {
    crate::api_v1::db::upsert_ping(device_id).await
}
