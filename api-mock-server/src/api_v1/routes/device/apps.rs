use amos_common::entities::ApplicationConfig;
use axum::{Json, http::StatusCode, response::IntoResponse};

use crate::auth::extractors::AuthDevice;

/// GET /device/apps - Get the assigned applications
pub async fn get(AuthDevice(device): AuthDevice) -> Result<impl IntoResponse, StatusCode> {
    let apps = match apps_get_assigned(device.id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("{:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(Json(
        apps.into_iter()
            .map(|a| amos_common::device_api::apps::GetResponseItem {
                id: a.id,
                application_id: a.application_id,
                image: a.image,
                config: a.config,
            })
            .collect::<Vec<_>>(),
    ))
}

/// PUT /device/apps - Report the currently running applications
pub async fn put(
    AuthDevice(device): AuthDevice,
    Json(body): Json<amos_common::device_api::apps::PutBody>,
) -> StatusCode {
    let config_ids = body.into_iter().map(|item| item.application_config_id);
    match apps_put_report(device.id, config_ids).await {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn apps_get_assigned(
    device_id: i32,
) -> Result<Vec<ApplicationConfig::Model>, sea_orm::DbErr> {
    crate::api_v1::db::list_application_configs_for_device(device_id).await
}

async fn apps_put_report(
    device_id: i32,
    application_config_ids: impl Iterator<Item = i32>,
) -> Result<(), sea_orm::DbErr> {
    for config_id in application_config_ids {
        crate::api_v1::db::add_reported_application_assignment(config_id, device_id).await?;
    }

    Ok(())
}
