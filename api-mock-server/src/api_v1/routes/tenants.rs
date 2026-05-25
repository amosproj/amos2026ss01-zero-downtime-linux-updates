use crate::api_v1::db;
use amos_common::entities::Tenant;
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
        .route("/tenants", get(list_tenants).post(create_tenant))
        .route(
            "/tenants/{id}",
            get(get_tenant).put(update_tenant).delete(delete_tenant),
        )
}

/// GET /tenants — List all tenants.
async fn list_tenants() -> Response {
    match db::list_tenants().await {
        Ok(tenants) => Json(tenants).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /tenants/{id} — Get a tenant by ID.
async fn get_tenant(Path(id): Path<i32>) -> Response {
    match db::get_tenant(id).await {
        Ok(Some(tenant)) => Json(tenant).into_response(),
        Ok(None) => not_found("Tenant", id),
        Err(e) => db_err(e),
    }
}

/// POST /tenants — Create a tenant.
/// Body: `{ name: string (required), description: string|null }`
async fn create_tenant(Json(body): Json<Tenant::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Tenant name cannot be empty",
        );
    }
    match db::add_tenant(body.name, body.description).await {
        Ok(tenant) => (StatusCode::CREATED, Json(tenant)).into_response(),
        Err(e) => db_err(e),
    }
}

/// PUT /tenants/{id} — Replace a tenant by ID.
/// Body: `{ name: string (required), description: string|null }`
async fn update_tenant(Path(id): Path<i32>, Json(body): Json<Tenant::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Tenant name cannot be empty",
        );
    }
    match db::update_tenant(id, body.name, body.description).await {
        Ok(tenant) => Json(tenant).into_response(),
        Err(e) => db_err(e),
    }
}

/// DELETE /tenants/{id} — Delete a tenant by ID. Returns 204 on success.
async fn delete_tenant(Path(id): Path<i32>) -> Response {
    match db::delete_tenant(id).await {
        Ok(0) => not_found("Tenant", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => db_err(e),
    }
}
