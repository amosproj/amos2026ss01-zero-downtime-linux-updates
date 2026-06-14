#![allow(dead_code)]

use crate::dtos;
use amos_common::entities::AuditLog;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

fn into_api(model: dtos::AuditLog::Model) -> AuditLog::Model {
    AuditLog::Model {
        id: model.id,
        table_name: model.table_name,
        record_id: model.record_id,
        operation: model.operation,
        old_data: model.old_data,
        new_data: model.new_data,
        changed_by: model.changed_by,
        changed_at: model.changed_at,
    }
}

pub async fn list_audit_logs(
    table_name: Option<String>,
    record_id: Option<String>,
    changed_by: Option<String>,
    operation: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<AuditLog::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::AuditLog::Entity::find().order_by_desc(dtos::AuditLog::Column::ChangedAt);

    if let Some(tn) = table_name {
        query = query.filter(dtos::AuditLog::Column::TableName.eq(tn));
    }
    if let Some(rid) = record_id {
        query = query.filter(dtos::AuditLog::Column::RecordId.eq(rid));
    }
    if let Some(cb) = changed_by {
        query = query.filter(dtos::AuditLog::Column::ChangedBy.eq(cb));
    }
    if let Some(op) = operation {
        query = query.filter(dtos::AuditLog::Column::Operation.eq(op));
    }

    let paginator = query.paginate(&db, page_size);
    let total = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;

    Ok((data.into_iter().map(into_api).collect(), total))
}

pub async fn get_audit_logs_for_record(
    table_name: &str,
    record_id: &str,
) -> Result<Vec<AuditLog::Model>, DbErr> {
    let db = db!();
    let data = dtos::AuditLog::Entity::find()
        .filter(dtos::AuditLog::Column::TableName.eq(table_name))
        .filter(dtos::AuditLog::Column::RecordId.eq(record_id))
        .order_by_asc(dtos::AuditLog::Column::ChangedAt)
        .all(&db)
        .await?;

    Ok(data.into_iter().map(into_api).collect())
}

pub async fn get_audit_logs_for_table(
    table_name: &str,
    page: u64,
    page_size: u64,
) -> Result<(Vec<AuditLog::Model>, u64), DbErr> {
    let db = db!();
    let query = dtos::AuditLog::Entity::find()
        .filter(dtos::AuditLog::Column::TableName.eq(table_name))
        .order_by_desc(dtos::AuditLog::Column::ChangedAt);

    let paginator = query.paginate(&db, page_size);
    let total = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;

    Ok((data.into_iter().map(into_api).collect(), total))
}

pub async fn get_audit_logs_for_device(
    device_id: i32,
    page: u64,
    page_size: u64,
) -> Result<(Vec<AuditLog::Model>, u64), DbErr> {
    let db = db!();
    let device_id_str = device_id.to_string();

    let is_device = Expr::col(dtos::AuditLog::Column::TableName)
        .eq(Expr::val("devices"))
        .and(Expr::col(dtos::AuditLog::Column::RecordId).eq(Expr::val(device_id_str.as_str())));

    // PostgreSQL-specific JSONB operators (->>) to search for device_id in
    // old_data/new_data columns. This function only works on PostgreSQL;
    // audit triggers do not fire on SQLite so there are no rows to query.
    let old_has_device =
        Expr::cust_with_values("old_data->>'device_id' = $1", [device_id_str.clone()]);
    let new_has_device =
        Expr::cust_with_values("new_data->>'device_id' = $1", [device_id_str.clone()]);

    let query = dtos::AuditLog::Entity::find()
        .filter(is_device.or(old_has_device).or(new_has_device))
        .order_by_desc(dtos::AuditLog::Column::ChangedAt);

    let paginator = query.paginate(&db, page_size);
    let total = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;

    Ok((data.into_iter().map(into_api).collect(), total))
}

pub async fn count_audit_logs_for(
    table_name: &str,
    record_id: &str,
    operation: &str,
) -> Result<u64, DbErr> {
    let db = db!();
    let count = dtos::AuditLog::Entity::find()
        .filter(dtos::AuditLog::Column::TableName.eq(table_name))
        .filter(dtos::AuditLog::Column::RecordId.eq(record_id))
        .filter(dtos::AuditLog::Column::Operation.eq(operation))
        .count(&db)
        .await?;

    Ok(count)
}
