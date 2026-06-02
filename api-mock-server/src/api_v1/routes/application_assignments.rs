use crate::api_v1::db;
use crate::api_v1::routes::pagination::{Page, PageParams};
use crate::api_v1::routes::{db_err, err, not_found, pagination_err};
use amos_common::entities::application_assignment::CreateModel as ApplicationAssignmentCreate;
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
            "/app-assignments",
            get(list_application_assignments).post(create_application_assignment),
        )
        .route(
            "/app-assignments/{id}",
            get(get_application_assignment)
                .put(update_application_assignment)
                .delete(delete_application_assignment),
        )
}

#[derive(Deserialize)]
struct AppAssignmentQuery {
    application_config_id: Option<i32>,
    device_id: Option<i32>,
    device_uuid: Option<String>,
    group_id: Option<i32>,
}

/// GET /app-assignments — List app assignments.
/// Optional query: `?application_config_id=<i32>&device_id=<i32>&device_uuid=<str>&group_id=<i32>&page=1&page_size=20`
async fn list_application_assignments(
    Query(page): Query<PageParams>,
    Query(params): Query<AppAssignmentQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }

    let mut device_id = params.device_id;

    if let Some(device_uuid) = params.device_uuid {
        match db::get_device_by_uuid(device_uuid.clone()).await {
            Ok(Some(device)) => device_id = Some(device.id),
            Ok(None) => {
                return err(
                    StatusCode::NOT_FOUND,
                    format!("No device with uuid {} found", device_uuid),
                );
            }
            Err(e) => return db_err(e),
        }
    }

    match db::list_application_assignments(
        params.application_config_id,
        device_id,
        params.group_id,
        page.to_db_page(),
        page.page_size,
    )
    .await
    {
        Ok((assignments, total)) => {
            Json(Page::new(assignments, page.page, page.page_size, total)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// GET /app-assignments/{id} — Get an app assignment by ID.
async fn get_application_assignment(Path(id): Path<i32>) -> Response {
    match db::get_application_assignment(id).await {
        Ok(Some(assignment)) => Json(assignment).into_response(),
        Ok(None) => not_found("ApplicationAssignment", id),
        Err(e) => db_err(e),
    }
}

/// POST /app-assignments — Create an app assignment.
/// Body: `{ application_config_id: i32, device_id: i32|null, group_id: i32|null }`
/// Exactly one of device_id or group_id must be set.
async fn create_application_assignment(Json(body): Json<ApplicationAssignmentCreate>) -> Response {
    if body.device_id.is_some() && body.group_id.is_some() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Only one of device_id or group_id may be set, not both",
        );
    }
    match (body.device_id, body.group_id) {
        (Some(device_id), None) => {
            match db::add_application_assignment_to_device(body.application_config_id, device_id)
                .await
            {
                Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
                Err(e) => db_err(e),
            }
        }
        (None, Some(group_id)) => {
            match db::add_application_assignment_to_group(body.application_config_id, group_id)
                .await
            {
                Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
                Err(e) => db_err(e),
            }
        }
        _ => err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        ),
    }
}

/// PUT /app-assignments/{id} — Replace an app assignment by ID.
/// Body: `{ application_config_id: i32, device_id: i32|null, group_id: i32|null }`
/// Exactly one of device_id or group_id must be set.
async fn update_application_assignment(
    Path(id): Path<i32>,
    Json(body): Json<ApplicationAssignmentCreate>,
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
    match db::update_application_assignment(
        id,
        body.application_config_id,
        body.device_id,
        body.group_id,
    )
    .await
    {
        Ok(assignment) => Json(assignment).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /app-assignments/{id} — Delete an app assignment by ID. Returns 204 on success.
async fn delete_application_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_application_assignment(id).await {
        Ok(0) => not_found("ApplicationAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
