use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::Application;
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes() -> Router {
    Router::new()
        .route(
            "/applications",
            get(list_applications).post(create_application),
        )
        .route(
            "/applications/{id}",
            get(get_application)
                .put(update_application)
                .delete(delete_application),
        )
}

#[derive(serde::Deserialize)]
struct ApplicationQuery {
    name: Option<String>,
}

/// GET /applications — List applications.
/// Optional query: `?name=<string>&page=1&page_size=20`
async fn list_applications(Query(page): Query<PageParams>, Query(params): Query<ApplicationQuery>) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_applications(params.name, page.to_db_page(), page.page_size).await
    {
        Ok((data, total)) => Json(Page::new(data, page, total)).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /applications/{id} — Get an application by ID.
async fn get_application(Path(id): Path<i32>) -> Response {
    match db::get_application(id).await {
        Ok(Some(app)) => Json(app).into_response(),
        Ok(None) => not_found("Application", id),
        Err(e) => db_err(e),
    }
}

/// POST /applications — Create an application.
/// Body: `{ name: string (required), description: string }`
async fn create_application(Json(body): Json<Application::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Application name cannot be empty",
        );
    }
    match db::add_application(body.name, body.description).await {
        Ok(app) => (StatusCode::CREATED, Json(app)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /applications/{id} — Replace an application by ID.
/// Body: `{ name: string (required), description: string }`
async fn update_application(Path(id): Path<i32>, Json(body): Json<Application::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Application name cannot be empty",
        );
    }
    match db::update_application(id, body.name, body.description).await {
        Ok(app) => Json(app).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /applications/{id} — Delete an application by ID. Returns 204 on success.
async fn delete_application(Path(id): Path<i32>) -> Response {
    match db::delete_application(id).await {
        Ok(0) => not_found("Application", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
