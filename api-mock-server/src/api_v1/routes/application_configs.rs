use crate::api_v1::db;
use crate::api_v1::routes::{db_err, err, not_found, pagination_err, pagination::{default_page, default_page_size, Page, PageParams}};
use amos_common::entities::ApplicationConfig;
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
            "/app-configs",
            get(list_application_configs).post(create_application_config),
        )
        .route(
            "/app-configs/{id}",
            get(get_application_config)
                .put(update_application_config)
                .delete(delete_application_config),
        )
}

#[derive(Deserialize)]
struct AppConfigQuery {
    application_id: Option<i32>,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
}

/// GET /app-configs — List app configs. 
/// Optional query: `?application_id=<i32>&page=1&page_size=20`
async fn list_application_configs(Query(params): Query<AppConfigQuery>) -> Response {
    let page_params = PageParams::new(params.page, params.page_size);
    if let Err(e) = page_params.validate() {
        return pagination_err(e);
    }
    match db::list_application_configs(params.application_id, page_params.to_db_page(), page_params.page_size).await {
        Ok((data, total)) => Json(Page::new(data, page_params, total)).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /app-configs/{id} — Get an app config by ID.
async fn get_application_config(Path(id): Path<i32>) -> Response {
    match db::get_application_config(id).await {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => not_found("ApplicationConfig", id),
        Err(e) => db_err(e),
    }
}

/// POST /app-configs — Create an app config.
/// Body: `{ application_id: i32, image: string (required), config: string|null, comment: string|null }`
async fn create_application_config(Json(body): Json<ApplicationConfig::Model>) -> Response {
    if body.image.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ApplicationConfig image cannot be empty",
        );
    }
    match db::add_application_config(body.application_id, body.image, body.config, body.comment)
        .await
    {
        Ok(config) => (StatusCode::CREATED, Json(config)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /app-configs/{id} — Replace an app config by ID.
/// Body: `{ application_id: i32, image: string (required), config: string|null, comment: string|null }`
async fn update_application_config(
    Path(id): Path<i32>,
    Json(body): Json<ApplicationConfig::Model>,
) -> Response {
    if body.image.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ApplicationConfig image cannot be empty",
        );
    }
    match db::update_application_config(
        id,
        body.application_id,
        body.image,
        body.config,
        body.comment,
    )
    .await
    {
        Ok(config) => Json(config).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /app-configs/{id} — Delete an app config by ID. Returns 204 on success.
async fn delete_application_config(Path(id): Path<i32>) -> Response {
    match db::delete_application_config(id).await {
        Ok(0) => not_found("ApplicationConfig", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
