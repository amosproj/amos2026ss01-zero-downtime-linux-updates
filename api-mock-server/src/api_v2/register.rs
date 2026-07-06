use axum::{Json, extract::State, http::StatusCode};

use crate::api_v2::db::DataStore;

/// POST /register - Try and register a device with the server.
/// Can only succeed if there is a pending registration.
pub async fn post(
    State(db): State<DataStore>,
    Json(body): Json<amos_common::device_api::register::PostBody>,
) -> StatusCode {
    let err_msg = {
        if body.uuid.trim().is_empty() {
            Some("Device UUID cannot be empty")
        } else if body.serial_number.trim().is_empty() {
            Some("Device serial number cannot be empty")
        } else if body.endorsement_public_key.trim().is_empty() {
            Some("Device endorsement key cannot be empty")
        } else if body.signing_public_key.trim().is_empty() {
            Some("Device signing key cannot be empty")
        } else {
            None
        }
    };

    if let Some(msg) = err_msg {
        log::error!("Could not register device: {:?}", msg);
        return StatusCode::BAD_REQUEST;
    }

    let result = db
        .register_device(
            body.uuid,
            body.serial_number,
            body.endorsement_public_key,
            body.signing_public_key,
        )
        .await;

    match result {
        Ok(_) => StatusCode::CREATED,
        Err(sea_orm::DbErr::RecordNotFound(_)) => StatusCode::BAD_REQUEST,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
