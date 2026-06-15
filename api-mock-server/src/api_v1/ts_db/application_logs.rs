use crate::dtos;
use amos_common::entities::ApplicationLog;
use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{DbErr, EntityTrait};
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
