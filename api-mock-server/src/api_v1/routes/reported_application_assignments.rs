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
    // No POST or PUT — reported assignments should come from devices
    Router::new()
        .route(
            "/reported-app-assignments",
            get(list_reported_application_assignments),
        )
        .route(
            "/reported-app-assignments/{id}",
            get(get_reported_application_assignment).delete(delete_reported_application_assignment),
        )
}

#[derive(Deserialize)]
struct ReportedAppAssignmentQuery {
    device_id: Option<i32>,
    application_config_id: Option<i32>,
}

/// GET /reported-app-assignments — List reported app assignments (current device state).
/// Optional query: `?device_id=<i32>&application_config_id=<i32>&page=1&page_size=20`
async fn list_reported_application_assignments(
    Query(page): Query<PageParams>,
    Query(params): Query<ReportedAppAssignmentQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_reported_application_assignments(
        params.device_id,
        params.application_config_id,
        page.to_db_page(),
        page.page_size,
    )
    .await
    {
        Ok((data, total)) => Json(Page::new(data, page, total)).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /reported-app-assignments/{id} — Get a reported app assignment by ID.
async fn get_reported_application_assignment(Path(id): Path<i32>) -> Response {
    match db::get_reported_application_assignment(id).await {
        Ok(Some(a)) => Json(a).into_response(),
        Ok(None) => not_found("ReportedApplicationAssignment", id),
        Err(e) => db_err(e),
    }
}

/// DELETE /reported-app-assignments/{id} — Delete a reported app assignment by ID. Returns 204 on success.
async fn delete_reported_application_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_reported_application_assignment(id).await {
        Ok(0) => not_found("ReportedApplicationAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
