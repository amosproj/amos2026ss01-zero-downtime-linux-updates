use crate::dtos;
use amos_common::entities::Ping;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{DbErr, EntityTrait, PaginatorTrait, QueryOrder};

use super::db;

// --Device Pings--

pub async fn list_pings(page: u64, page_size: u64) -> Result<(Vec<Ping::Model>, u64), DbErr> {
    let db = db!();
    let query = dtos::Ping::Entity::find().order_by_desc(dtos::Ping::Column::ReportedAt);
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn upsert_ping(device_id: i32, uptime_secs: Option<i64>) -> Result<(), DbErr> {
    let db = db!();

    let ping = dtos::Ping::ActiveModel {
        device_id: Set(device_id),
        reported_at: Set(chrono::Utc::now()),
        uptime_secs: Set(uptime_secs),
    };

    dtos::Ping::Entity::insert(ping)
        .on_conflict(
            OnConflict::column(dtos::Ping::Column::DeviceId)
                .update_columns([
                    dtos::Ping::Column::ReportedAt,
                    dtos::Ping::Column::UptimeSecs,
                ])
                .to_owned(),
        )
        .exec(&db)
        .await?;

    Ok(())
}
