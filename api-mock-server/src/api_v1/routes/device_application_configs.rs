use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::device_application_config::CreateModel as DeviceApplicationConfigCreate;
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
            "/device-app-configs",
            get(list_device_application_configs).post(create_device_application_config),
        )
        .route(
            "/device-app-configs/{id}",
            get(get_device_application_config)
                .put(update_device_application_config)
                .delete(delete_device_application_config),
        )
}

#[derive(Deserialize)]
struct DeviceApplicationConfigQuery {
    device_id: Option<i32>,
    application_id: Option<i32>,
}

/// GET /device-app-configs — List device application configs.
/// Optional query: `?device_id=<i32>&application_id=<i32>&page=1&page_size=20`
async fn list_device_application_configs(
    Query(page): Query<PageParams>,
    Query(params): Query<DeviceApplicationConfigQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_device_application_configs(
        params.device_id,
        params.application_id,
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

/// GET /device-app-configs/{id} — Get a device application config by ID.
async fn get_device_application_config(Path(id): Path<i32>) -> Response {
    match db::get_device_application_config(id).await {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => not_found("DeviceApplicationConfig", id),
        Err(e) => db_err(e),
    }
}

/// POST /device-app-configs — Create a device application config.
/// Body: `{ device_id: i32, application_id: i32, config: string (required), version: i32 (default 1) }`
async fn create_device_application_config(
    Json(body): Json<DeviceApplicationConfigCreate>,
) -> Response {
    if body.config.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "DeviceApplicationConfig config cannot be empty",
        );
    }
    match db::add_device_application_config(
        body.device_id,
        body.application_id,
        body.config,
        body.version,
    )
    .await
    {
        Ok(config) => (StatusCode::CREATED, Json(config)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /device-app-configs/{id} — Replace a device application config by ID.
/// Body: `{ device_id: i32, application_id: i32, config: string (required), version: i32 (default 1) }`
async fn update_device_application_config(
    Path(id): Path<i32>,
    Json(body): Json<DeviceApplicationConfigCreate>,
) -> Response {
    if body.config.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "DeviceApplicationConfig config cannot be empty",
        );
    }
    match db::update_device_application_config(
        id,
        body.device_id,
        body.application_id,
        body.config,
        body.version,
    )
    .await
    {
        Ok(config) => Json(config).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /device-app-configs/{id} — Delete a device application config by ID. Returns 204 on success.
async fn delete_device_application_config(Path(id): Path<i32>) -> Response {
    match db::delete_device_application_config(id).await {
        Ok(0) => not_found("DeviceApplicationConfig", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
