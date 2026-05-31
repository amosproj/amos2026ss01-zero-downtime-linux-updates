use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::{CreateGroup, Group};
use axum::{
    Json, Router,
    extract::{Path, Query},
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

#[derive(serde::Deserialize)]
struct GroupQuery {
    name: Option<String>,
}

/// GET /groups — List groups.
/// Optional query: `?name=<string>&page=1&page_size=20`
async fn list_groups(Query(page): Query<PageParams>, Query(params): Query<GroupQuery>) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_groups(params.name, page.to_db_page(), page.page_size).await {
        Ok((data, total_items)) => {
            Json(Page::new(data, page.page, page.page_size, total_items)).into_response()
        }
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
async fn create_group(Json(body): Json<CreateGroup>) -> Response {
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
