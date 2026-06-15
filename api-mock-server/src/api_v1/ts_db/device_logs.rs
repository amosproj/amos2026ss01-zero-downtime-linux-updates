use crate::dtos;
use amos_common::entities::DeviceLog;
use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{DbErr, EntityTrait};
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
