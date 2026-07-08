use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
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
            get(list_reported_os_assignments),
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

/// DELETE /reported-os-assignments/{id} — Delete a reported OS assignment by ID. Returns 204 on success.
async fn delete_reported_os_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_reported_os_assignment(id).await {
        Ok(0) => not_found("ReportedOsAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
