use std::convert::Infallible;
use std::time::Duration;

use crate::api_v1::log_stream;
use crate::api_v1::routes::{
    db_err,
    pagination::{Page, PageParams},
    pagination_err,
};
use crate::api_v1::ts_db;
use amos_common::entities::{LogEvent, LogKind, LogLevel, LogQuery, LogStreamQuery};
use axum::{
    Json, Router,
    extract::Query,
    response::sse::{Event, KeepAlive, Sse},
    response::{IntoResponse, Response},
    routing::get,
};
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;

pub fn routes() -> Router {
    Router::new()
        .route("/logs/devices", get(list_device_logs))
        .route("/logs/applications", get(list_application_logs))
        .route("/logs/stream", get(stream_logs))
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
    kind: Option<LogKind>,
) -> bool {
    if let Some(kind) = kind {
        let event_kind = match event {
            LogEvent::Device(_) => LogKind::Device,
            LogEvent::Application(_) => LogKind::Application,
        };
        if event_kind != kind {
            return false;
        }
    }

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
        .filter(move |event| {
            matches(
                event,
                params.device_id,
                params.application_id,
                params.level,
                params.kind,
            )
        })
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
        assert!(matches(&event, None, None, None, None));
    }

    #[test]
    fn matches_filters_by_device_id() {
        let event = device_log_event(1, LogLevel::Info);
        assert!(matches(&event, Some(1), None, None, None));
        assert!(!matches(&event, Some(2), None, None, None));
    }

    #[test]
    fn matches_filters_by_application_id() {
        let event = application_log_event(1, 5, LogLevel::Info);
        assert!(matches(&event, None, Some(5), None, None));
        assert!(!matches(&event, None, Some(6), None, None));
    }

    #[test]
    fn matches_application_id_filter_excludes_device_events() {
        let event = device_log_event(1, LogLevel::Info);
        assert!(!matches(&event, None, Some(5), None, None));
    }

    #[test]
    fn matches_filters_by_minimum_level() {
        let event = device_log_event(1, LogLevel::Warn);
        assert!(matches(&event, None, None, Some(LogLevel::Warn), None));
        assert!(matches(&event, None, None, Some(LogLevel::Info), None));
        assert!(!matches(&event, None, None, Some(LogLevel::Error), None));
    }

    #[test]
    fn matches_filters_by_kind_device() {
        let device_event = device_log_event(1, LogLevel::Info);
        let app_event = application_log_event(1, 5, LogLevel::Info);
        assert!(matches(
            &device_event,
            None,
            None,
            None,
            Some(LogKind::Device)
        ));
        assert!(!matches(
            &app_event,
            None,
            None,
            None,
            Some(LogKind::Device)
        ));
    }

    #[test]
    fn matches_filters_by_kind_application() {
        let device_event = device_log_event(1, LogLevel::Info);
        let app_event = application_log_event(1, 5, LogLevel::Info);
        assert!(matches(
            &app_event,
            None,
            None,
            None,
            Some(LogKind::Application)
        ));
        assert!(!matches(
            &device_event,
            None,
            None,
            None,
            Some(LogKind::Application)
        ));
    }
}
