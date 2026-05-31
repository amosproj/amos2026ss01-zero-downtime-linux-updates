use crate::api_v1::db;
use crate::api_v1::routes::{db_err, err, not_found};
use amos_common::entities::device::CreateModel as DeviceCreate;
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
}

/// GET /devices/summary — List all device summaries (reported state).
/// Optional query: `?tenant_id=<i32>`
async fn list_device_summaries(Query(params): Query<DeviceQuery>) -> Response {
    match db::list_device_summaries(params.tenant_id).await {
        Ok(summaries) => Json(summaries).into_response(),
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

/// GET /devices — List devices. Optional query: `?group_id=<i32>&tenant_id=<i32>`
async fn list_devices(Query(params): Query<DeviceQuery>) -> Response {
    match db::list_devices(params.group_id, params.tenant_id).await {
        Ok(devices) => Json(devices).into_response(),
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
async fn create_device(Json(body): Json<DeviceCreate>) -> Response {
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
async fn update_device(Path(id): Path<i32>, Json(body): Json<DeviceCreate>) -> Response {
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
