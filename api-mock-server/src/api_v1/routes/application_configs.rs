use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::application_config::CreateModel as ApplicationConfigCreate;
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
    device_id: Option<i32>,
    device_uuid: Option<String>,
    group_id: Option<i32>,
}

/// GET /app-configs — List app configs.
/// Optional query: `?application_id=<i32>&device_id=<i32>&group_id=<i32>&page=1&page_size=20`
/// `?device_uuid=<str>` resolves the effective configs for that device: a
/// device-specific config supersedes a config assigned to the device's group.
async fn list_application_configs(
    Query(page): Query<PageParams>,
    Query(params): Query<AppConfigQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }

    if let Some(device_uuid) = params.device_uuid {
        let device = match db::get_device_by_uuid(device_uuid.clone()).await {
            Ok(Some(device)) => device,
            Ok(None) => {
                return err(
                    StatusCode::NOT_FOUND,
                    format!("No device with uuid {} found", device_uuid),
                );
            }
            Err(e) => return db_err(e),
        };
        return match db::list_application_configs_for_device(device.id).await {
            Ok(data) => {
                let total = data.len() as u64;
                Json(Page::new(data, 1, total.max(1), total)).into_response()
            }
            Err(e) => db_err(e),
        };
    }

    match db::list_application_configs(
        params.application_id,
        params.device_id,
        params.group_id,
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

/// GET /app-configs/{id} — Get an app config by ID.
async fn get_application_config(Path(id): Path<i32>) -> Response {
    match db::get_application_config(id).await {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => not_found("ApplicationConfig", id),
        Err(e) => db_err(e),
    }
}

/// POST /app-configs — Create an app config.
/// Body: `{ device_id: i32|null, group_id: i32|null, application_id: i32, image: string (required), config: string (required), version: i32 (default 1) }`
/// Exactly one of device_id or group_id must be set.
async fn create_application_config(Json(body): Json<ApplicationConfigCreate>) -> Response {
    if let Some(e) = validate_app_config_body(&body) {
        return e;
    }
    match db::add_application_config(
        body.device_id,
        body.group_id,
        body.application_id,
        body.image,
        body.config,
    )
    .await
    {
        Ok(config) => (StatusCode::CREATED, Json(config)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /app-configs/{id} — Replace an app config by ID.
/// Body: `{ device_id: i32|null, group_id: i32|null, application_id: i32, image: string (required), config: string (required), version: i32 (default 1) }`
/// Exactly one of device_id or group_id must be set.
async fn update_application_config(
    Path(id): Path<i32>,
    Json(body): Json<ApplicationConfigCreate>,
) -> Response {
    if let Some(e) = validate_app_config_body(&body) {
        return e;
    }
    match db::update_application_config(
        id,
        body.device_id,
        body.group_id,
        body.application_id,
        body.image,
        body.config,
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

fn validate_app_config_body(body: &ApplicationConfigCreate) -> Option<Response> {
    if body.image.trim().is_empty() {
        return Some(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ApplicationConfig image cannot be empty",
        ));
    }
    if body.device_id.is_none() && body.group_id.is_none() {
        return Some(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        ));
    }
    if body.device_id.is_some() && body.group_id.is_some() {
        return Some(err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Only one of device_id or group_id may be set, not both",
        ));
    }
    None
}
