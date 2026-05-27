use crate::api_v1::db;
use crate::api_v1::routes::{db_err, err};
use axum::{
    Json, Router,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, put},
};

pub fn routes() -> Router {
    Router::new()
        .route("/pings", get(list_pings))
        .route("/pings/{device_uuid}", put(upsert_ping))
}

/// GET /pings — List device pings.
async fn list_pings() -> Response {
    match db::list_pings().await {
        Ok(assignments) => Json(assignments).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /pings/{device_uuid} — Create/update a device ping.
async fn upsert_ping(Path(device_uuid): Path<String>) -> Response {
    let device_id = match db::get_device_by_uuid(device_uuid.clone()).await {
        Ok(Some(device)) => device.id,
        Ok(None) => {
            return err(
                StatusCode::NOT_FOUND,
                format!("No device with uuid {} found", device_uuid),
            );
        }
        Err(e) => return db_err(e),
    };

    match db::upsert_ping(device_id).await {
        Ok(_) => (StatusCode::NO_CONTENT).into_response(),
        Err(e) => db_err(e),
    }
}
