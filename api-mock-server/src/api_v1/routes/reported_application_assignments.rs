use crate::api_v1::db;
use crate::api_v1::routes::{db_err, err, not_found};
use amos_common::entities::ReportedApplicationAssignment;
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
            "/reported-app-assignments",
            get(list_reported_application_assignments).post(create_reported_application_assignment),
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

#[derive(Deserialize)]
struct CreateReportedAppAssignmentQuery {
    device_uuid: Option<String>,
}

/// GET /reported-app-assignments — List reported app assignments (current device state).
/// Optional query: `?device_id=<i32>&application_config_id=<i32>`
async fn list_reported_application_assignments(
    Query(params): Query<ReportedAppAssignmentQuery>,
) -> Response {
    match db::list_reported_application_assignments(params.device_id, params.application_config_id)
        .await
    {
        Ok(assignments) => Json(assignments).into_response(),
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

/// POST /reported-app-assignments — Create a reported app assignment.
/// Optional query: `?device_uuid=<str>` to resolve device_id from a device UUID.
/// Body: `{ application_config_id: i32, device_id: i32, ... }` (ReportedApplicationAssignment::Model)
async fn create_reported_application_assignment(
    Query(params): Query<CreateReportedAppAssignmentQuery>,
    Json(body): Json<ReportedApplicationAssignment::Model>,
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

    match db::add_reported_application_assignment(body.application_config_id, device_id).await {
        Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
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
