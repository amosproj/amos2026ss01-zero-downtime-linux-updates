use axum::{extract::State, http::StatusCode};

use crate::{api_v2::db::DataStore, auth::extractors::AuthDevice};

/// PUT /device/ping - Send aliveness signal
pub async fn put(State(db): State<DataStore>, AuthDevice(device): AuthDevice) -> StatusCode {
    match db.ping_upsert(device.id).await {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
