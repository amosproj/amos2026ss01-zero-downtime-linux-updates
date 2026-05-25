use crate::api_v1::db;
use amos_common::entities::OsVersion;
use amos_common::http_errors::{db_err, err, not_found};
use axum::{
    Json, Router,
    extract::Path,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes() -> Router {
    Router::new()
        .route(
            "/os-versions",
            get(list_os_versions).post(create_os_version),
        )
        .route(
            "/os-versions/{id}",
            get(get_os_version)
                .put(update_os_version)
                .delete(delete_os_version),
        )
}

/// GET /os-versions — List all OS versions.
async fn list_os_versions() -> Response {
    match db::list_os_versions().await {
        Ok(versions) => Json(versions).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /os-versions/{id} — Get an OS version by ID.
async fn get_os_version(Path(id): Path<i32>) -> Response {
    match db::get_os_version(id).await {
        Ok(Some(v)) => Json(v).into_response(),
        Ok(None) => not_found("OsVersion", id),
        Err(e) => db_err(e),
    }
}

/// POST /os-versions — Create an OS version.
/// Body: `{ commit_hash: string (required), orchestrator_version: string (required), description: string|null }`
async fn create_os_version(Json(body): Json<OsVersion::Model>) -> Response {
    if body.commit_hash.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version commit hash cannot be empty",
        );
    }
    if body.orchestrator_version.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version orchestrator version cannot be empty",
        );
    }
    match db::add_os_version(
        body.commit_hash,
        body.orchestrator_version,
        body.description,
    )
    .await
    {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /os-versions/{id} — Replace an OS version by ID.
/// Body: `{ commit_hash: string (required), orchestrator_version: string (required), description: string|null }`
async fn update_os_version(Path(id): Path<i32>, Json(body): Json<OsVersion::Model>) -> Response {
    if body.commit_hash.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version commit hash cannot be empty",
        );
    }
    if body.orchestrator_version.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version orchestrator version cannot be empty",
        );
    }
    match db::update_os_version(
        id,
        body.commit_hash,
        body.orchestrator_version,
        body.description,
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /os-versions/{id} — Delete an OS version by ID. Returns 204 on success.
async fn delete_os_version(Path(id): Path<i32>) -> Response {
    match db::delete_os_version(id).await {
        Ok(0) => not_found("OsVersion", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
