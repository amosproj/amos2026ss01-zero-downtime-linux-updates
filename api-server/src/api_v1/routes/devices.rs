use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::device::{CreateModel as DeviceCreate, UpdateModel as DeviceUpdate};
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use sea_orm::DbErr;
use serde::Deserialize;

pub fn routes() -> Router {
    Router::new()
        .route("/devices/summary", get(list_device_summaries))
        .route("/devices/{id}/summary", get(get_device_summary))
        .route("/devices", get(list_devices).post(create_device))
        .route(
            "/devices/{id}",
            get(get_device)
                .put(update_device)
                .patch(patch_device)
                .delete(delete_device),
        )
}

#[derive(Deserialize)]
struct DeviceQuery {
    group_id: Option<i32>,
    tenant_id: Option<i32>,
    uuid: Option<String>,
    serial_number: Option<String>,
}

/// GET /devices/summary — List device summaries (reported state).
/// Optional query: `?group_id=<i32>&tenant_id=<i32>&uuid=<string>&serial_number=<string>&page=1&page_size=20`
async fn list_device_summaries(
    Query(page): Query<PageParams>,
    Query(params): Query<DeviceQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_device_summaries(
        params.group_id,
        params.tenant_id,
        params.uuid,
        params.serial_number,
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

/// GET /devices/{id}/summary — Get a single device summary (reported state) by device ID.
async fn get_device_summary(Path(id): Path<i32>) -> Response {
    match db::get_device_summary(id).await {
        Ok(Some(summary)) => Json(summary).into_response(),
        Ok(None) => not_found("Device", id),
        Err(e) => db_err(e),
    }
}

/// GET /devices — List devices.
/// Optional query: `?group_id=<i32>&tenant_id=<i32>&uuid=<string>&serial_number=<string>&page=1&page_size=20`
async fn list_devices(
    Query(page): Query<PageParams>,
    Query(params): Query<DeviceQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_devices(
        params.group_id,
        params.tenant_id,
        params.uuid,
        params.serial_number,
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

/// GET /devices/{id} — Get a device by ID.
async fn get_device(Path(id): Path<i32>) -> Response {
    match db::get_device(id).await {
        Ok(Some(device)) => Json(device).into_response(),
        Ok(None) => not_found("Device", id),
        Err(e) => db_err(e),
    }
}

/// POST /devices — Create a device.
/// Body: `{ uuid: string (required), serial_number: string (required), tenant_id: i32, group_id: i32|null }`
async fn create_device(Json(body): Json<DeviceCreate>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device UUID cannot be empty",
        );
    }
    if body.serial_number.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device serial number cannot be empty",
        );
    }
    match db::add_device(
        body.uuid,
        body.public_key,
        body.serial_number,
        body.tenant_id,
        body.group_id,
    )
    .await
    {
        Ok(device) => (StatusCode::CREATED, Json(device)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /devices/{id} — Replace a device by ID.
/// Body: `{ uuid: string (required), serial_number: string (required), tenant_id: i32, group_id: i32|null }`
async fn update_device(Path(id): Path<i32>, Json(body): Json<DeviceCreate>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device UUID cannot be empty",
        );
    }
    if body.serial_number.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device serial number cannot be empty",
        );
    }
    match db::update_device(
        id,
        body.uuid,
        body.public_key,
        body.serial_number,
        body.tenant_id,
        body.group_id,
    )
    .await
    {
        Ok(device) => Json(device).into_response(),
        Err(e) => db_err(e),
    }
}

/// PATCH /devices/{id} — Update a device by ID.
/// Body: see amos_common::entities::device::UpdateModel
async fn patch_device(Path(id): Path<i32>, Json(body): Json<DeviceUpdate>) -> Response {
    match db::patch_device(
        id,
        body.uuid,
        body.public_key,
        body.serial_number,
        body.tenant_id,
        body.group_id,
    )
    .await
    {
        Ok(device) => Json(device).into_response(),
        Err(DbErr::RecordNotFound(_)) => err(
            StatusCode::NOT_FOUND,
            format!("No device with id {} found", id),
        ),
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
