use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::reported_os_assignment::CreateModel as ReportedOsAssignmentCreate;
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route(
            "/reported-os-assignments",
            get(list_reported_os_assignments).post(create_reported_os_assignment),
        )
        .route(
            "/reported-os-assignments/{id}",
            get(get_reported_os_assignment).delete(delete_reported_os_assignment),
        )
}

#[derive(Deserialize)]
struct ReportedOsAssignmentQuery {
    device_id: Option<i32>,
    os_version_id: Option<i32>,
}

#[derive(Deserialize)]
struct CreateReportedOsAssignmentQuery {
    device_uuid: Option<String>,
}

/// GET /reported-os-assignments — List reported OS assignments (current device state).
/// Optional query: `?device_id=<i32>&os_version_id=<i32>&page=1&page_size=20`
async fn list_reported_os_assignments(
    Query(page): Query<PageParams>,
    Query(params): Query<ReportedOsAssignmentQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_reported_os_assignments(
        params.device_id,
        params.os_version_id,
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

/// GET /reported-os-assignments/{id} — Get a reported OS assignment by ID.
async fn get_reported_os_assignment(Path(id): Path<i32>) -> Response {
    match db::get_reported_os_assignment(id).await {
        Ok(Some(a)) => Json(a).into_response(),
        Ok(None) => not_found("ReportedOsAssignment", id),
        Err(e) => db_err(e),
    }
}

/// POST /reported-os-assignments — Create a reported OS assignment.
/// Optional query: `?device_uuid=<str>` to resolve device_id from a device UUID.
/// Body: `{ os_version_id: i32, device_id: i32|null }` — device_id can be omitted when device_uuid query param is provided.
async fn create_reported_os_assignment(
    Query(params): Query<CreateReportedOsAssignmentQuery>,
    Json(body): Json<ReportedOsAssignmentCreate>,
) -> Response {
    let device_id = if let Some(uuid) = params.device_uuid {
        match db::get_device_by_uuid(uuid.clone()).await {
            Ok(Some(device)) => device.id,
            Ok(None) => {
                return err(
                    StatusCode::NOT_FOUND,
                    format!("No device with uuid {} found", uuid),
                );
            }
            Err(e) => return db_err(e),
        }
    } else {
        match body.device_id {
            Some(id) => id,
            None => {
                return err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "Either device_id in body or device_uuid query param must be provided",
                );
            }
        }
    };

    match db::add_reported_os_assignment(body.os_version_id, device_id).await {
        Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /reported-os-assignments/{id} — Delete a reported OS assignment by ID. Returns 204 on success.
async fn delete_reported_os_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_reported_os_assignment(id).await {
        Ok(0) => not_found("ReportedOsAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
