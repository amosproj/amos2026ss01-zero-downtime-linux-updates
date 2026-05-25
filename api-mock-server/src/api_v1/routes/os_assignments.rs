use crate::api_v1::db;
use amos_common::entities::OsAssignment;
use amos_common::http_errors::{db_err, err, not_found};
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
            "/os-assignments",
            get(list_os_assignments).post(create_os_assignment),
        )
        .route(
            "/os-assignments/{id}",
            get(get_os_assignment)
                .put(update_os_assignment)
                .delete(delete_os_assignment),
        )
}

#[derive(Deserialize)]
struct OsAssignmentQuery {
    os_version_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
}

/// GET /os-assignments — List OS assignments (target state).
/// Optional query: `?os_version_id=<i32>&device_id=<i32>&group_id=<i32>`
async fn list_os_assignments(Query(params): Query<OsAssignmentQuery>) -> Response {
    match db::list_os_assignments(params.os_version_id, params.device_id, params.group_id).await {
        Ok(assignments) => Json(assignments).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /os-assignments/{id} — Get an OS assignment by ID.
async fn get_os_assignment(Path(id): Path<i32>) -> Response {
    match db::get_os_assignment(id).await {
        Ok(Some(a)) => Json(a).into_response(),
        Ok(None) => not_found("OsAssignment", id),
        Err(e) => db_err(e),
    }
}

/// POST /os-assignments — Create an OS assignment.
/// Body: `{ os_version_id: i32, device_id: i32|null, group_id: i32|null }`
/// Exactly one of device_id or group_id must be set.
async fn create_os_assignment(Json(body): Json<OsAssignment::Model>) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        );
    }
    if body.device_id.is_some() && body.group_id.is_some() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Only one of device_id or group_id may be set, not both",
        );
    }
    match db::add_os_assignment(body.os_version_id, body.device_id, body.group_id).await {
        Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /os-assignments/{id} — Replace an OS assignment by ID.
/// Body: `{ os_version_id: i32, device_id: i32|null, group_id: i32|null }`
/// Exactly one of device_id or group_id must be set.
async fn update_os_assignment(
    Path(id): Path<i32>,
    Json(body): Json<OsAssignment::Model>,
) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        );
    }
    if body.device_id.is_some() && body.group_id.is_some() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Only one of device_id or group_id may be set, not both",
        );
    }
    match db::update_os_assignment(id, body.os_version_id, body.device_id, body.group_id).await {
        Ok(a) => Json(a).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /os-assignments/{id} — Delete an OS assignment by ID. Returns 204 on success.
async fn delete_os_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_os_assignment(id).await {
        Ok(0) => not_found("OsAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
