use crate::api_v1::db;
use crate::api_v1::routes::{
    db_err, not_found,
    pagination::{Page, PageParams},
    pagination_err,
};
use axum::{
    Json, Router,
    extract::{Path, Query},
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes() -> Router {
    Router::new()
        .route("/audit-logs", get(list_audit_logs))
        .route(
            "/audit-logs/{table_name}/{record_id}",
            get(get_audit_logs_for_record),
        )
        .route("/audit-logs/by-device/{id}", get(get_audit_logs_for_device))
}

#[derive(serde::Deserialize)]
struct AuditLogQuery {
    table_name: Option<String>,
    record_id: Option<String>,
    changed_by: Option<i32>,
    operation: Option<String>,
}

/// GET /audit-logs — List audit log entries.
/// Optional query: `?table_name=<string>&record_id=<string>&changed_by=<int>&operation=<string>&page=1&page_size=20`
async fn list_audit_logs(
    Query(page): Query<PageParams>,
    Query(params): Query<AuditLogQuery>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::audit_log::list_audit_logs(
        params.table_name,
        params.record_id,
        params.changed_by,
        params.operation,
        page.to_db_page(),
        page.page_size,
    )
    .await
    {
        Ok((data, total_items)) => {
            Json(Page::new(data, page.page, page.page_size, total_items)).into_response()
        }
        Err(e) => db_err(e),
    }
}

/// GET /audit-logs/{table_name}/{record_id} — Full audit history for a specific record.
async fn get_audit_logs_for_record(
    Path((table_name, record_id)): Path<(String, String)>,
) -> Response {
    match db::audit_log::get_audit_logs_for_record(&table_name, &record_id).await {
        Ok(entries) if entries.is_empty() => not_found("AuditLog", record_id),
        Ok(entries) => Json(entries).into_response(),
        Err(e) => db_err(e),
    }
}

/// GET /audit-logs/by-device/{id} — Audit history for a device, including related
/// application/OS assignment changes.
/// Optional query: `?page=1&page_size=20`
async fn get_audit_logs_for_device(
    Path(id): Path<i32>,
    Query(page): Query<PageParams>,
) -> Response {
    if let Err(e) = page.validate() {
        return pagination_err(e);
    }
    match db::audit_log::get_audit_logs_for_device(id, page.to_db_page(), page.page_size).await {
        Ok((data, total_items)) => {
            Json(Page::new(data, page.page, page.page_size, total_items)).into_response()
        }
        Err(e) => db_err(e),
    }
}
