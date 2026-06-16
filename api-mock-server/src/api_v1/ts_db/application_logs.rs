use crate::dtos;
use amos_common::entities::{ApplicationLog, LogLevel};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::Set;
use sea_orm::{ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use super::ts_db;

pub async fn insert_application_log_entries(
    device_id: i32,
    application_id: i32,
    entries: Vec<ApplicationLog::CreateEntry>,
) -> Result<Vec<ApplicationLog::Model>, DbErr> {
    let db = ts_db!();

    let models: Vec<dtos::ApplicationLog::Model> = entries
        .into_iter()
        .map(|entry| dtos::ApplicationLog::Model {
            id: Uuid::now_v7(),
            time: entry.time.unwrap_or_else(Utc::now),
            device_id,
            application_id,
            level: entry.level.into(),
            message: entry.message,
            source: entry.source,
        })
        .collect();

    let active_models: Vec<dtos::ApplicationLog::ActiveModel> = models
        .iter()
        .cloned()
        .map(|m| dtos::ApplicationLog::ActiveModel {
            time: Set(m.time),
            id: Set(m.id),
            device_id: Set(m.device_id),
            application_id: Set(m.application_id),
            level: Set(m.level),
            message: Set(m.message),
            source: Set(m.source),
        })
        .collect();

    dtos::ApplicationLog::Entity::insert_many(active_models)
        .exec(&db)
        .await?;

    Ok(models.into_iter().map(|m| m.into_api()).collect())
}

/// Lists historic application log entries, most recent first, optionally
/// filtered by device, application, minimum severity and/or time range.
pub async fn list_application_logs(
    device_id: Option<i32>,
    application_id: Option<i32>,
    min_level: Option<LogLevel>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<ApplicationLog::Model>, u64), DbErr> {
    let db = ts_db!();

    let mut query =
        dtos::ApplicationLog::Entity::find().order_by_desc(dtos::ApplicationLog::Column::Time);

    if let Some(device_id) = device_id {
        query = query.filter(dtos::ApplicationLog::Column::DeviceId.eq(device_id));
    }
    if let Some(application_id) = application_id {
        query = query.filter(dtos::ApplicationLog::Column::ApplicationId.eq(application_id));
    }
    if let Some(min_level) = min_level {
        query = query
            .filter(dtos::ApplicationLog::Column::Level.is_in(super::levels_at_least(min_level)));
    }
    if let Some(from) = from {
        query = query.filter(dtos::ApplicationLog::Column::Time.gte(from));
    }
    if let Some(to) = to {
        query = query.filter(dtos::ApplicationLog::Column::Time.lte(to));
    }

    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;

    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}
