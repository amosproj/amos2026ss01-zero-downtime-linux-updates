use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, err,
    pagination::{Page, PageParams},
    pagination_err,
};
use axum::{
    Json, Router,
    extract::{Path, Query},
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
/// Optional query: `?page=1&page_size=20`
async fn list_pings(Query(page): Query<PageParams>) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::list_pings(page.to_db_page(), page.page_size).await {
        Ok((data, total)) => {
            Json(Page::new(data, page.page, page.page_size, total)).into_response()
        }
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
