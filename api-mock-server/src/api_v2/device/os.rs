use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{api_v2::db::DataStore, auth::extractors::AuthDevice};

/// GET /device/os - Get the assigned OS version
pub async fn get(State(db): State<DataStore>, AuthDevice(device): AuthDevice) -> Response {
    let os = match db.os_get_assigned(device.id, device.group_id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("{:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(amos_common::device_api::os::GetResponse {
        id: os.id,
        commit_hash: os.commit_hash,
        orchestrator_version: os.orchestrator_version,
        description: os.description,
    })
    .into_response()
}

/// PUT /device/os - Report the currently running OS version
pub async fn put(
    State(db): State<DataStore>,
    AuthDevice(device): AuthDevice,
    Json(body): Json<amos_common::device_api::os::PutBody>,
) -> StatusCode {
    match db.os_put_report(device.id, body.os_version_id).await {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
