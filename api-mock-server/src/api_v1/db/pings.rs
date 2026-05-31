use amos_common::entities::Ping;
use sea_orm::ActiveValue::Set;
use sea_orm::sea_query::OnConflict;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{DbErr, EntityTrait, PaginatorTrait, QueryOrder};

use super::db;

// --Device Pings--

pub async fn list_pings(page: u64, page_size: u64) -> Result<(Vec<Ping::Model>, u64), DbErr> {
    let db = db!();
    let query = Ping::Entity::find().order_by_desc(Ping::Column::ReportedAt);
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn upsert_ping(device_id: i32) -> Result<(), DbErr> {
    let db = db!();

    let ping = Ping::ActiveModel {
        device_id: Set(device_id),
        reported_at: Set(chrono::Utc::now()),
    };

    Ping::Entity::insert(ping)
        .on_conflict(
            OnConflict::column(Ping::Column::DeviceId)
                .update_column(Ping::Column::ReportedAt)
                .to_owned(),
        )
        .exec(&db)
        .await?;

    Ok(())
}
