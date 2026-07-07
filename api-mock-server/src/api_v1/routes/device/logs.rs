use amos_common::entities::{ApplicationLog, DeviceLog, LogEvent};
use axum::{
    Json,
    extract::Query,
    http::StatusCode,
};

use crate::auth::extractors::AuthDevice;

/// POST /device/logs - Publish some log lines
pub async fn post(
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
            logs_publish_application(device.id, app_id, entries).await
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
            logs_publish_device(device.id, entries).await
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

async fn logs_publish_device(
    device_id: i32,
    entries: Vec<DeviceLog::CreateEntry>,
) -> Result<(), sea_orm::DbErr> {
    let rows = crate::api_v1::ts_db::insert_device_log_entries(device_id, entries).await?;

    // Send log lines to real-time subscribers
    for row in rows {
        crate::api_v1::log_stream::publish(LogEvent::Device(row));
    }

    Ok(())
}

async fn logs_publish_application(
    device_id: i32,
    application_id: i32,
    entries: Vec<ApplicationLog::CreateEntry>,
) -> Result<(), sea_orm::DbErr> {
    let rows =
        crate::api_v1::ts_db::insert_application_log_entries(device_id, application_id, entries)
            .await?;

    // Send log lines to real-time subscribers
    for row in rows {
        crate::api_v1::log_stream::publish(LogEvent::Application(row));
    }

    Ok(())
}
