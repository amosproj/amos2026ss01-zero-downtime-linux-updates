use amos_common::entities::OsVersion;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::auth::extractors::AuthDevice;

/// GET /device/os - Get the assigned OS version
pub async fn get(AuthDevice(device): AuthDevice) -> Response {
    let os = match os_get_assigned(device.id, device.group_id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("{:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(amos_common::device_api::os::GetResponse {
        id: os.id,
        commit_hash: os.commit_hash,
    })
    .into_response()
}

/// PUT /device/os - Report the currently running OS version
pub async fn put(
    AuthDevice(device): AuthDevice,
    Json(body): Json<amos_common::device_api::os::PutBody>,
) -> StatusCode {
    match os_put_report(device.id, body.os_version_id).await {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn os_get_assigned(
    device_id: i32,
    group_id: Option<i32>,
) -> Result<OsVersion::Model, sea_orm::DbErr> {
    let (assignments, _) =
        crate::api_v1::db::list_os_assignments_for_device(device_id, group_id, None, 0, u64::MAX)
            .await?;

    if assignments.is_empty() {
        return Err(sea_orm::DbErr::RecordNotFound("OsVersion".to_owned()));
    }

    match crate::api_v1::db::get_os_version(assignments[0].os_version_id).await? {
        Some(ver) => Ok(ver),
        None => Err(sea_orm::DbErr::RecordNotFound("OsVersion".to_owned())),
    }
}

async fn os_put_report(device_id: i32, os_version_id: i32) -> Result<(), sea_orm::DbErr> {
    let _ = crate::api_v1::db::add_reported_os_assignment(os_version_id, device_id).await?;
    Ok(())
}
