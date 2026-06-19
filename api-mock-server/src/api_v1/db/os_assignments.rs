use crate::dtos;
use amos_common::entities::OsAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --OS Assignments--

pub async fn list_os_assignments(
    os_version_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<OsAssignment::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::OsAssignment::Entity::find()
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null())
        .order_by_asc(dtos::OsAssignment::Column::Id);
    if let Some(id) = os_version_id {
        query = query.filter(dtos::OsAssignment::Column::OsVersionId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(dtos::OsAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(dtos::OsAssignment::Column::GroupId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_os_assignment(id: i32) -> Result<Option<OsAssignment::Model>, DbErr> {
    let db = db!();
    Ok(dtos::OsAssignment::Entity::find_by_id(id)
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_os_assignment(
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<OsAssignment::Model, DbErr> {
    let os_assignment = dtos::OsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_os_assignment = os_assignment.insert(&db).await?;
    debug!(
        "Inserted new OS version assignment: {:?}",
        new_os_assignment
    );

    Ok(new_os_assignment.into_api())
}

pub async fn update_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<OsAssignment::Model, DbErr> {
    let db = db!();

    let current = dtos::OsAssignment::Entity::find_by_id(id)
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("OsAssignment not found".into()))?;

    let new_assignment = dtos::OsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };
    let new_assignment = new_assignment.insert(&db).await?;

    let old_active = dtos::OsAssignment::ActiveModel {
        id: Set(current.id),
        os_version_id: Set(current.os_version_id),
        device_id: Set(current.device_id),
        group_id: Set(current.group_id),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(Some(new_assignment.id)),
    };
    old_active.update(&db).await?;

    debug!(
        "Updated OS assignment via append-only: {:?}",
        new_assignment
    );
    Ok(new_assignment.into_api())
}

pub async fn delete_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::OsAssignment::Entity::find_by_id(id)
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(assignment) => {
            let active = dtos::OsAssignment::ActiveModel {
                id: Set(assignment.id),
                os_version_id: Set(assignment.os_version_id),
                device_id: Set(assignment.device_id),
                group_id: Set(assignment.group_id),
                deleted_at: Set(Some(chrono::Utc::now())),
                superseded_by: Set(assignment.superseded_by),
            };
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
