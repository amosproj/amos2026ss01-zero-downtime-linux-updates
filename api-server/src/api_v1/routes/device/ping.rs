use axum::Json;
use axum::http::StatusCode;

use crate::auth::extractors::AuthDevice;

/// PUT /device/ping - Send aliveness signal
pub async fn put(
    AuthDevice(device): AuthDevice,
    body: Option<Json<amos_common::device_api::ping::PutBody>>,
) -> StatusCode {
    let uptime_secs = body.map(|Json(b)| b.uptime_secs);
    match ping_upsert(device.id, uptime_secs).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

async fn ping_upsert(device_id: i32, uptime_secs: Option<i64>) -> Result<(), sea_orm::DbErr> {
    crate::api_v1::db::upsert_ping(device_id, uptime_secs).await
}
