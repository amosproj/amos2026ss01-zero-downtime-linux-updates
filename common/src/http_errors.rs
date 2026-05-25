use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

pub fn err(status: StatusCode, message: impl std::fmt::Display) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

pub fn not_found(resource: &str, id: i32) -> Response {
    err(
        StatusCode::NOT_FOUND,
        format!("{} with id {} not found", resource, id),
    )
}

pub fn db_err(e: sea_orm::DbErr) -> Response {
    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Database error: {}", e),
    )
}
