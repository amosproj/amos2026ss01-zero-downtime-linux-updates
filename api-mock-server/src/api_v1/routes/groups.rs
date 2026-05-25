use crate::api_v1::db;
use crate::api_v1::routes::{db_err, err, not_found};
use amos_common::entities::Group;
use axum::{
    Json, Router,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes() -> Router {
    Router::new()
        .route("/groups", get(list_groups).post(create_group))
        .route(
            "/groups/{id}",
            get(get_group).put(update_group).delete(delete_group),
        )
}

/// GET /groups — List all groups.
async fn list_groups() -> Response {
    match db::list_groups().await {
        Ok(groups) => Json(groups).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /groups/{id} — Get a group by ID.
async fn get_group(Path(id): Path<i32>) -> Response {
    match db::get_group(id).await {
        Ok(Some(group)) => Json(group).into_response(),
        Ok(None) => not_found("Group", id),
        Err(e) => db_err(e),
    }
}

/// POST /groups — Create a group.
/// Body: `{ name: string (required) }`
async fn create_group(Json(body): Json<Group::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Group name cannot be empty",
        );
    }
    match db::add_group(body.name).await {
        Ok(group) => (StatusCode::CREATED, Json(group)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /groups/{id} — Replace a group by ID.
/// Body: `{ name: string (required) }`
async fn update_group(Path(id): Path<i32>, Json(body): Json<Group::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Group name cannot be empty",
        );
    }
    match db::update_group(id, body.name).await {
        Ok(group) => Json(group).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /groups/{id} — Delete a group by ID. Returns 204 on success.
async fn delete_group(Path(id): Path<i32>) -> Response {
    match db::delete_group(id).await {
        Ok(0) => not_found("Group", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
