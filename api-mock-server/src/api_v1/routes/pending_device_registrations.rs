use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use amos_common::entities::pending_device_registration::CreateModel as PendingDeviceRegistrationCreate;
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get},
};

pub fn routes() -> Router {
    Router::new()
        .route(
            "/pending-device-registrations",
            get(list_pending_device_registrations).post(create_pending_device_registration),
        )
        .route(
            "/pending-device-registrations/{id}",
            delete(delete_pending_device_registration),
        )
}

/// GET /pending-device-registration — List pending device registrations.
/// Optional query: `?page=1&page_size=20`
async fn list_pending_device_registrations(Query(page): Query<PageParams>) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_pending_device_registrations(page.to_db_page(), page.page_size).await {
        Ok((data, total)) => {
            Json(Page::new(data, page.page, page.page_size, total)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// POST /pending-device-registration — Create a pending device registration.
/// Body: `{ serial_number: str, endorsement_public_key: str }`
async fn create_pending_device_registration(
    Json(body): Json<PendingDeviceRegistrationCreate>,
) -> Response {
    match db::add_pending_device_registration(body.serial_number, body.endorsement_public_key).await
    {
        Ok(a) => (StatusCode::CREATED, Json(a)).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /pending-device-registration/{id} — Delete a pending device registration by ID. Returns 204 on success.
async fn delete_pending_device_registration(Path(id): Path<i32>) -> Response {
    match db::delete_pending_device_registration(id).await {
        Ok(0) => not_found("PendingDeviceRegistration", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
