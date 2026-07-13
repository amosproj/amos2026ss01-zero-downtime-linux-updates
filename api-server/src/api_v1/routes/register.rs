use axum::{Json, http::StatusCode};
use sea_orm::ModelTrait;

/// POST /register - Try and register a device with the server.
/// Can only succeed if there is a pending registration.
pub async fn post(Json(body): Json<amos_common::device_api::register::PostBody>) -> StatusCode {
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
        log::warn!("Could not register device: {}", msg);
        return StatusCode::BAD_REQUEST;
    }

    let result = register_device(
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

async fn register_device(
    uuid: String,
    serial_number: String,
    endorsement_pubkey: String,
    signing_pubkey: String,
) -> Result<(), sea_orm::DbErr> {
    // Check if a matching pending registration is in the database
    let found = crate::api_v1::db::search_pending_device_registration(
        serial_number.clone(),
        endorsement_pubkey,
    )
    .await?;

    let active = match found {
        Some(x) => x,
        None => {
            return Err(sea_orm::DbErr::RecordNotFound(
                "Did not find device registration".to_owned(),
            ));
        }
    };

    let new_device = crate::api_v1::db::add_device(
        uuid,
        Some(signing_pubkey),
        serial_number,
        1, // TODO: Having to guess a tenat here is BAD, tho not sure what else to do as it is mandatory
        None,
    )
    .await?;

    log::info!(
        "New device registered successfully: {}, SN: {}",
        new_device.uuid,
        new_device.serial_number
    );

    let _ = active.delete(&crate::api_v1::db::db!()).await;
    Ok(())
}
