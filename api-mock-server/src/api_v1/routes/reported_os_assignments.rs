use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams, default_page, default_page_size},
    pagination_err,
};
use amos_common::entities::ReportedOsAssignment;
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
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
}

#[derive(Deserialize)]
struct CreateReportedOsAssignmentQuery {
    device_uuid: Option<String>,
}

/// GET /reported-os-assignments — List reported OS assignments (current device state).
/// Optional query: `?device_id=<i32>&os_version_id=<i32>&page=1&page_size=20`
async fn list_reported_os_assignments(Query(params): Query<ReportedOsAssignmentQuery>) -> Response {
    let page_params = PageParams::new(params.page, params.page_size);
    if let Err(e) = page_params.validate() {
        return pagination_err(e);
    }
    match db::list_reported_os_assignments(
        params.device_id,
        params.os_version_id,
        page_params.to_db_page(),
        page_params.page_size,
    )
    .await
    {
        Ok((data, total)) => Json(Page::new(data, page_params, total)).into_response(),
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
/// Body: `{ os_version_id: i32, device_id: i32, ... }` (ReportedOsAssignment::Model)
async fn create_reported_os_assignment(
    Query(params): Query<CreateReportedOsAssignmentQuery>,
    Json(body): Json<ReportedOsAssignment::Model>,
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
        body.device_id
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
