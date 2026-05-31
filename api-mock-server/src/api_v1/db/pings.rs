use crate::dtos;
use amos_common::entities::Ping;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{DbErr, EntityTrait};

use super::db;

// --Device Pings--

pub async fn list_pings() -> Result<Vec<Ping::Model>, DbErr> {
    let db = db!();
    dtos::Ping::Entity::find()
        .all(&db)
        .await
        .map(|v| v.into_iter().map(|m| m.into_api()).collect())
}

pub async fn upsert_ping(device_id: i32) -> Result<(), DbErr> {
    let db = db!();

    let ping = dtos::Ping::ActiveModel {
        device_id: Set(device_id),
        reported_at: Set(chrono::Utc::now()),
    };

    dtos::Ping::Entity::insert(ping)
        .on_conflict(
            OnConflict::column(dtos::Ping::Column::DeviceId)
                .update_column(dtos::Ping::Column::ReportedAt)
                .to_owned(),
        )
        .exec(&db)
        .await?;

    Ok(())
}
