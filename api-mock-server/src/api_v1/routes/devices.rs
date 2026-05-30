use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams, default_page, default_page_size},
    pagination_err,
};
use amos_common::entities::Device;
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
        .route("/devices/summary", get(list_device_summaries))
        .route("/devices/{id}/summary", get(get_device_summary))
        .route("/devices", get(list_devices).post(create_device))
        .route(
            "/devices/{id}",
            get(get_device).put(update_device).delete(delete_device),
        )
}

#[derive(Deserialize)]
struct DeviceQuery {
    group_id: Option<i32>,
    tenant_id: Option<i32>,
    uuid: Option<String>,
    hostname: Option<String>,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
}

/// GET /devices/summary — List device summaries (reported state).
/// Optional query: `?group_id=<i32>&tenant_id=<i32>&uuid=<string>&hostname=<string>&page=1&page_size=20`
async fn list_device_summaries(Query(params): Query<DeviceQuery>) -> Response {
    let page_params = PageParams::new(params.page, params.page_size);
    if let Err(e) = page_params.validate() {
        return pagination_err(e);
    }
    match db::list_device_summaries(
        params.group_id,
        params.tenant_id,
        params.uuid,
        params.hostname,
        page_params.to_db_page(),
        page_params.page_size,
    )
    .await
    {
        Ok((data, total)) => Json(Page::new(data, page_params, total)).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /devices/{id}/summary — Get a single device summary (reported state) by device ID.
async fn get_device_summary(Path(id): Path<i32>) -> Response {
    match db::get_device_summary(id).await {
        Ok(Some(summary)) => Json(summary).into_response(),
        Ok(None) => not_found("Device", id),
        Err(e) => db_err(e),
    }
}

/// GET /devices — List devices.
/// Optional query: `?group_id=<i32>&tenant_id=<i32>&uuid=<string>&hostname=<string>&page=1&page_size=20`
async fn list_devices(Query(params): Query<DeviceQuery>) -> Response {
    let page_params = PageParams::new(params.page, params.page_size);
    if let Err(e) = page_params.validate() {
        return pagination_err(e);
    }
    match db::list_devices(
        params.group_id,
        params.tenant_id,
        params.uuid,
        params.hostname,
        page_params.to_db_page(),
        page_params.page_size,
    )
    .await
    {
        Ok((data, total)) => Json(Page::new(data, page_params, total)).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /devices/{id} — Get a device by ID.
async fn get_device(Path(id): Path<i32>) -> Response {
    match db::get_device(id).await {
        Ok(Some(device)) => Json(device).into_response(),
        Ok(None) => not_found("Device", id),
        Err(e) => db_err(e),
    }
}

/// POST /devices — Create a device.
/// Body: `{ uuid: string (required), hostname: string (required), tenant_id: i32, group_id: i32|null }`
async fn create_device(Json(body): Json<Device::Model>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device UUID cannot be empty",
        );
    }
    if body.hostname.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device hostname cannot be empty",
        );
    }
    match db::add_device(body.uuid, body.hostname, body.tenant_id, body.group_id).await {
        Ok(device) => (StatusCode::CREATED, Json(device)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /devices/{id} — Replace a device by ID.
/// Body: `{ uuid: string (required), hostname: string (required), tenant_id: i32, group_id: i32|null }`
async fn update_device(Path(id): Path<i32>, Json(body): Json<Device::Model>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device UUID cannot be empty",
        );
    }
    if body.hostname.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device hostname cannot be empty",
        );
    }
    match db::update_device(id, body.uuid, body.hostname, body.tenant_id, body.group_id).await {
        Ok(device) => Json(device).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /devices/{id} — Delete a device by ID. Returns 204 on success.
async fn delete_device(Path(id): Path<i32>) -> Response {
    match db::delete_device(id).await {
        Ok(0) => not_found("Device", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
