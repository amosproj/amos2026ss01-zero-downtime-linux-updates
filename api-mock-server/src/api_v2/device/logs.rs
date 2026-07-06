use amos_common::entities::{ApplicationLog, DeviceLog};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};

use crate::{api_v2::db::DataStore, auth::extractors::AuthDevice};

/// POST /device/logs - Publish some log lines
pub async fn post(
    State(db): State<DataStore>,
    AuthDevice(device): AuthDevice,
    Query(params): Query<amos_common::device_api::logs::PostQueryParams>,
    Json(body): Json<amos_common::device_api::logs::PostBody>,
) -> StatusCode {
    let result = match params.application_id {
        Some(app_id) => {
            let entries = body
                .into_iter()
                .map(|item| ApplicationLog::CreateEntry {
                    time: item.time,
                    level: item.level,
                    message: item.message,
                    source: item.source,
                })
                .collect::<Vec<_>>();
            db.logs_publish_application(device.id, app_id, entries)
                .await
        }
        None => {
            let entries = body
                .into_iter()
                .map(|item| DeviceLog::CreateEntry {
                    time: item.time,
                    level: item.level,
                    message: item.message,
                    source: item.source,
                })
                .collect::<Vec<_>>();
            db.logs_publish_device(device.id, entries).await
        }
    };

    match result {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}
