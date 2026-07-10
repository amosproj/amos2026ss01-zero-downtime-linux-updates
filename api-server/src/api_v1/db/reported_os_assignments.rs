use crate::dtos;
use amos_common::entities::ReportedOsAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Reported OS Assignments--

pub async fn list_reported_os_assignments(
    device_id: Option<i32>,
    os_version_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<ReportedOsAssignment::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::ReportedOsAssignment::Entity::find()
        .order_by_asc(dtos::ReportedOsAssignment::Column::Id);
    if let Some(id) = device_id {
        query = query.filter(dtos::ReportedOsAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = os_version_id {
        query = query.filter(dtos::ReportedOsAssignment::Column::OsVersionId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_reported_os_assignment(
    id: i32,
) -> Result<Option<ReportedOsAssignment::Model>, DbErr> {
    let db = db!();
    Ok(dtos::ReportedOsAssignment::Entity::find_by_id(id)
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_reported_os_assignment(os_version_id: i32, device_id: i32) -> Result<(), DbErr> {
    let os_assignment = dtos::ReportedOsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: Set(chrono::Utc::now()),
    };

    let db = db!();

    dtos::ReportedOsAssignment::Entity::insert(os_assignment)
        .on_conflict(
            OnConflict::columns([
                dtos::ReportedOsAssignment::Column::DeviceId,
                dtos::ReportedOsAssignment::Column::OsVersionId,
            ])
            .update_column(dtos::ReportedOsAssignment::Column::UpdatedAt)
            .to_owned(),
        )
        .exec(&db)
        .await?;

    debug!(
        "Inserted new reported OS version assignment: device={} version={}",
        device_id, os_version_id
    );
    Ok(())
}

#[allow(dead_code)]
pub async fn update_reported_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let db = db!();
    let os_assignment = dtos::ReportedOsAssignment::Entity::find_by_id(id)
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "ReportedOsAssignment not found".into(),
        ))?;
    let mut os_assignment: dtos::ReportedOsAssignment::ActiveModel = os_assignment.into();
    os_assignment.os_version_id = Set(os_version_id);
    os_assignment.device_id = Set(device_id);
    let updated = os_assignment.update(&db).await?;
    debug!("Updated reported OS version assignment: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_reported_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::ReportedOsAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
