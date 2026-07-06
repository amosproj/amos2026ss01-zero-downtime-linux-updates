use axum::{Json, extract::State, http::StatusCode};

use crate::{api_v2::db::DataStore, auth::extractors::AuthDevice};

/// GET /device/os - Get the assigned OS version
pub async fn get(State(db): State<DataStore>, AuthDevice(device): AuthDevice) -> StatusCode {
    StatusCode::OK
}

/// PUT /device/os - Report the currently running OS version
pub async fn put(
    State(db): State<DataStore>,
    AuthDevice(device): AuthDevice,
    Json(body): Json<amos_common::device_api::os::PutBody>,
) -> StatusCode {
    StatusCode::CREATED
}
