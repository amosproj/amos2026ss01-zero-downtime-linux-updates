use crate::dtos;
use amos_common::entities::{DeviceLog, LogLevel};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::ts_db;

pub async fn insert_device_log_entries(
    device_id: i32,
    entries: Vec<DeviceLog::CreateEntry>,
) -> Result<Vec<DeviceLog::Model>, DbErr> {
    let db = ts_db!();

    let models: Vec<dtos::DeviceLog::Model> = entries
        .into_iter()
        .map(|entry| dtos::DeviceLog::Model {
            id: Uuid::now_v7(),
            time: entry.time.unwrap_or_else(Utc::now),
            device_id,
            level: entry.level.into(),
            message: entry.message,
            source: entry.source,
        })
        .collect();

    let active_models: Vec<dtos::DeviceLog::ActiveModel> = models
        .iter()
        .cloned()
        .map(|m| dtos::DeviceLog::ActiveModel {
            time: Set(m.time),
            id: Set(m.id),
            device_id: Set(m.device_id),
            level: Set(m.level),
            message: Set(m.message),
            source: Set(m.source),
        })
        .collect();

    dtos::DeviceLog::Entity::insert_many(active_models)
        .exec(&db)
        .await?;

    Ok(models.into_iter().map(|m| m.into_api()).collect())
}

/// Lists historic device log entries, most recent first, optionally filtered
/// by device, minimum severity and/or time range.
pub async fn list_device_logs(
    device_id: Option<i32>,
    min_level: Option<LogLevel>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<DeviceLog::Model>, u64), DbErr> {
    let db = ts_db!();

    let mut query = dtos::DeviceLog::Entity::find().order_by_desc(dtos::DeviceLog::Column::Time);

    if let Some(device_id) = device_id {
        query = query.filter(dtos::DeviceLog::Column::DeviceId.eq(device_id));
    }
    if let Some(min_level) = min_level {
        query = query.filter(dtos::DeviceLog::Column::Level.is_in(super::levels_at_least(min_level)));
    }
    if let Some(from) = from {
        query = query.filter(dtos::DeviceLog::Column::Time.gte(from));
    }
    if let Some(to) = to {
        query = query.filter(dtos::DeviceLog::Column::Time.lte(to));
    }

    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;

    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}
