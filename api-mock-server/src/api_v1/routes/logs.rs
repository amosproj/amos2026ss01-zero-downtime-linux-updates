use std::convert::Infallible;
use std::time::Duration;

use crate::api_v1::db;
use crate::api_v1::log_stream;
use crate::api_v1::routes::{
    db_err, err,
    pagination::{Page, PageParams},
    pagination_err,
};
use crate::api_v1::ts_db;
use amos_common::entities::{
    ApplicationLog, DeviceLog, LogEvent, LogLevel, LogQuery, LogStreamQuery,
};
use axum::{
    Json, Router,
    extract::Query,
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub fn routes() -> Router {
    Router::new()
        .route(
            "/logs/devices",
            get(list_device_logs).post(create_device_logs),
        )
        .route(
            "/logs/applications",
            get(list_application_logs).post(create_application_logs),
        )
        .route("/logs/stream", get(stream_logs))
}

#[derive(Deserialize)]
struct DeviceUuidQuery {
    device_uuid: Option<String>,
}

/// POST /logs/devices?device_uuid=<uuid> — Publish device log entries.
async fn create_device_logs(
    Query(params): Query<DeviceUuidQuery>,
    Json(body): Json<DeviceLog::CreateModel>,
) -> Response {
    let Some(device_uuid) = params.device_uuid else {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "device_uuid query param must be provided",
        );
    };

    if body.entries.is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "entries must not be empty",
        );
    }

    let device_id = match db::get_device_by_uuid(device_uuid.clone()).await {
        Ok(Some(device)) => device.id,
        Ok(None) => {
            return err(
                StatusCode::NOT_FOUND,
                format!("No device with uuid {} found", device_uuid),
            );
        }
        Err(e) => return db_err(e),
    };

    match ts_db::insert_device_log_entries(device_id, body.entries).await {
        Ok(entries) => {
            for entry in &entries {
                log_stream::publish(LogEvent::Device(entry.clone()));
            }
            (StatusCode::CREATED, Json(entries)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// POST /logs/applications?device_uuid=<uuid> — Publish application container log entries.
async fn create_application_logs(
    Query(params): Query<DeviceUuidQuery>,
    Json(body): Json<ApplicationLog::CreateModel>,
) -> Response {
    let Some(device_uuid) = params.device_uuid else {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "device_uuid query param must be provided",
        );
    };

    if body.entries.is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "entries must not be empty",
        );
    }

    let device_id = match db::get_device_by_uuid(device_uuid.clone()).await {
        Ok(Some(device)) => device.id,
        Ok(None) => {
            return err(
                StatusCode::NOT_FOUND,
                format!("No device with uuid {} found", device_uuid),
            );
        }
        Err(e) => return db_err(e),
    };

    match ts_db::insert_application_log_entries(device_id, body.application_id, body.entries).await
    {
        Ok(entries) => {
            for entry in &entries {
                log_stream::publish(LogEvent::Application(entry.clone()));
            }
            (StatusCode::CREATED, Json(entries)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// GET /logs/devices?device_id=&level=&from=&to=&page=&page_size= — Query historic device logs.
async fn list_device_logs(
    Query(page): Query<PageParams>,
    Query(params): Query<LogQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }

    match ts_db::list_device_logs(
        params.device_id,
        params.level,
        params.from,
        params.to,
        page.to_db_page(),
        page.page_size,
    )
    .await
    {
        Ok((data, total)) => {
            Json(Page::new(data, page.page, page.page_size, total)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// GET /logs/applications?device_id=&application_id=&level=&from=&to=&page=&page_size= — Query historic application logs.
async fn list_application_logs(
    Query(page): Query<PageParams>,
    Query(params): Query<LogQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }

    match ts_db::list_application_logs(
        params.device_id,
        params.application_id,
        params.level,
        params.from,
        params.to,
        page.to_db_page(),
        page.page_size,
    )
    .await
    {
        Ok((data, total)) => {
            Json(Page::new(data, page.page, page.page_size, total)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// Returns true if `event` matches the given filters.
///
/// `min_level` is a minimum severity filter: events with a level lower than
/// `min_level` are excluded.
pub(super) fn matches(
    event: &LogEvent,
    device_id: Option<i32>,
    application_id: Option<i32>,
    min_level: Option<LogLevel>,
) -> bool {
    if let Some(device_id) = device_id
        && event.device_id() != device_id
    {
        return false;
    }

    if let Some(application_id) = application_id
        && event.application_id() != Some(application_id)
    {
        return false;
    }

    if let Some(min_level) = min_level
        && event.level() < min_level
    {
        return false;
    }

    true
}

/// GET /logs/stream?device_id=&application_id=&level= — SSE stream of incoming logs.
async fn stream_logs(
    Query(params): Query<LogStreamQuery>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = log_stream::sender().subscribe();
    let stream = BroadcastStream::new(rx)
        .filter_map(|item: Result<LogEvent, BroadcastStreamRecvError>| item.ok())
        .filter(move |event| matches(event, params.device_id, params.application_id, params.level))
        .map(|event| Ok(Event::default().json_data(&event).unwrap()));

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use amos_common::entities::{ApplicationLog, DeviceLog};
    use chrono::Utc;
    use uuid::Uuid;

    fn device_log_event(device_id: i32, level: LogLevel) -> LogEvent {
        LogEvent::Device(DeviceLog::Model {
            id: Uuid::now_v7(),
            time: Utc::now(),
            device_id,
            level,
            message: "msg".into(),
            source: None,
        })
    }

    fn application_log_event(device_id: i32, application_id: i32, level: LogLevel) -> LogEvent {
        LogEvent::Application(ApplicationLog::Model {
            id: Uuid::now_v7(),
            time: Utc::now(),
            device_id,
            application_id,
            level,
            message: "msg".into(),
            source: None,
        })
    }

    #[test]
    fn matches_with_no_filters_returns_true() {
        let event = device_log_event(1, LogLevel::Info);
        assert!(matches(&event, None, None, None));
    }

    #[test]
    fn matches_filters_by_device_id() {
        let event = device_log_event(1, LogLevel::Info);
        assert!(matches(&event, Some(1), None, None));
        assert!(!matches(&event, Some(2), None, None));
    }

    #[test]
    fn matches_filters_by_application_id() {
        let event = application_log_event(1, 5, LogLevel::Info);
        assert!(matches(&event, None, Some(5), None));
        assert!(!matches(&event, None, Some(6), None));
    }

    #[test]
    fn matches_application_id_filter_excludes_device_events() {
        let event = device_log_event(1, LogLevel::Info);
        assert!(!matches(&event, None, Some(5), None));
    }

    #[test]
    fn matches_filters_by_minimum_level() {
        let event = device_log_event(1, LogLevel::Warn);
        assert!(matches(&event, None, None, Some(LogLevel::Warn)));
        assert!(matches(&event, None, None, Some(LogLevel::Info)));
        assert!(!matches(&event, None, None, Some(LogLevel::Error)));
    }
}
