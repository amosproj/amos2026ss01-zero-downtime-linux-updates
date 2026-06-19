use crate::dtos;
use amos_common::entities::ReportedOsAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
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
        .filter(dtos::ReportedOsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedOsAssignment::Column::SupersededBy.is_null())
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
        .filter(dtos::ReportedOsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedOsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_reported_os_assignment(
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let os_assignment = dtos::ReportedOsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: NotSet,
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_os_assignment = os_assignment.insert(&db).await?;
    debug!(
        "Inserted new reported OS version assignment: {:?}",
        new_os_assignment
    );
    Ok(new_os_assignment.into_api())
}

#[allow(dead_code)]
pub async fn update_reported_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let db = db!();

    let current = dtos::ReportedOsAssignment::Entity::find_by_id(id)
        .filter(dtos::ReportedOsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedOsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "ReportedOsAssignment not found".into(),
        ))?;

    let active = dtos::ReportedOsAssignment::ActiveModel {
        id: Set(current.id),
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: Set(current.updated_at),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(current.superseded_by),
    };
    let updated = active.update(&db).await?;
    debug!("Updated reported OS assignment: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_reported_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::ReportedOsAssignment::Entity::find_by_id(id)
        .filter(dtos::ReportedOsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedOsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(assignment) => {
            let active = dtos::ReportedOsAssignment::ActiveModel {
                id: Set(assignment.id),
                os_version_id: Set(assignment.os_version_id),
                device_id: Set(assignment.device_id),
                updated_at: Set(assignment.updated_at),
                deleted_at: Set(Some(chrono::Utc::now())),
                superseded_by: Set(assignment.superseded_by),
            };
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
