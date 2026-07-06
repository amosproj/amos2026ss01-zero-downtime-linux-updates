use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{api_v2::db::DataStore, auth::extractors::AuthDevice};

/// GET /device/apps - Get the assigned applications
pub async fn get(State(db): State<DataStore>, AuthDevice(device): AuthDevice) -> Response {
    let apps = match db.apps_get_assigned(device.id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("{:?}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    Json(
        apps.into_iter()
            .map(|a| amos_common::device_api::apps::GetResponseItem {
                id: a.id,
                application_id: a.application_id,
                image: a.image,
                config: a.config,
            })
            .collect::<Vec<_>>(),
    )
    .into_response()
}

/// PUT /device/apps - Report the currently running applications
pub async fn put(
    State(db): State<DataStore>,
    AuthDevice(device): AuthDevice,
    Json(body): Json<amos_common::device_api::apps::PutBody>,
) -> StatusCode {
    let config_ids = body.into_iter().map(|item| item.application_config_id);
    match db.apps_put_report(device.id, config_ids).await {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
