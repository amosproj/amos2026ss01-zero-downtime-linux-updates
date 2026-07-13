use crate::dtos;
use amos_common::entities::OsAssignment;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::TransactionTrait;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
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

// This should turn into device only route
pub async fn list_os_assignments_for_device(
    device_id: i32,
    group_id: Option<i32>,
    os_version_id: Option<i32>,
    _page: u64,
    _page_size: u64,
) -> Result<(Vec<OsAssignment::Model>, u64), DbErr> {
    let db = db!();

    let mut applies_to_device =
        Condition::any().add(dtos::OsAssignment::Column::DeviceId.eq(device_id));
    if let Some(gid) = group_id {
        applies_to_device = applies_to_device.add(dtos::OsAssignment::Column::GroupId.eq(gid));
    }

    let mut query = dtos::OsAssignment::Entity::find()
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null())
        // Sort by rows with device_id to give them priority over ones with group_id
        .order_by_desc(dtos::OsAssignment::Column::DeviceId)
        .order_by_asc(dtos::OsAssignment::Column::Id)
        .filter(applies_to_device);

    if let Some(id) = os_version_id {
        query = query.filter(dtos::OsAssignment::Column::OsVersionId.eq(id));
    }

    let all = query.all(&db).await?;
    let winner = all.into_iter().next();
    let (paged, total_items) = match winner {
        Some(m) => (vec![m.into_api()], 1),
        None => (vec![], 0),
    };
    Ok((paged, total_items))
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
    immediate: Option<bool>,
) -> Result<OsAssignment::Model, DbErr> {
    let db = db!();
    let txn = db.begin().await?;

    let mut old_query = dtos::OsAssignment::Entity::find()
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null());

    if let Some(did) = device_id {
        old_query = old_query.filter(dtos::OsAssignment::Column::DeviceId.eq(did));
    } else if let Some(gid) = group_id {
        old_query = old_query.filter(dtos::OsAssignment::Column::GroupId.eq(gid));
    }

    let existing_assignments = old_query.all(&txn).await?;

    let os_assignment = dtos::OsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        immediate: Set(immediate.unwrap_or(false)),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let new_os_assignment = os_assignment.insert(&txn).await?;
    log::trace!(
        "Inserted new OS version assignment: {:?}",
        new_os_assignment
    );

    for old in existing_assignments {
        let old_id = old.id;
        let mut old_active: dtos::OsAssignment::ActiveModel = old.into();
        old_active.superseded_by = Set(Some(new_os_assignment.id));
        old_active.update(&txn).await?;
        log::trace!(
            "Superseded old OS assignment {} with {}",
            old_id,
            new_os_assignment.id
        );
    }

    txn.commit().await?;

    Ok(new_os_assignment.into_api())
}

pub async fn update_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
    immediate: Option<bool>,
) -> Result<OsAssignment::Model, DbErr> {
    let db = db!();

    let current = dtos::OsAssignment::Entity::find_by_id(id)
        .filter(dtos::OsAssignment::Column::DeletedAt.is_null())
        .filter(dtos::OsAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("OsAssignment not found".into()))?;

    let immediate_value = match immediate {
        Some(v) => Set(v),
        None => NotSet,
    };

    let new_assignment = dtos::OsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        immediate: immediate_value,
        deleted_at: NotSet,
        superseded_by: NotSet,
    };
    let new_assignment = new_assignment.insert(&db).await?;

    let mut old_active: dtos::OsAssignment::ActiveModel = current.into();
    old_active.superseded_by = Set(Some(new_assignment.id));
    old_active.update(&db).await?;

    log::trace!(
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
            let mut active: dtos::OsAssignment::ActiveModel = assignment.into();
            active.deleted_at = Set(Some(chrono::Utc::now()));
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
