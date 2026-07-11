use crate::dtos;
use amos_common::entities::ApplicationAssignment;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbErr, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::db;

// --Application Assignments--

pub async fn list_application_assignments(
    application_config_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<ApplicationAssignment::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::ApplicationAssignment::Entity::find()
        .filter(dtos::ApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationAssignment::Column::SupersededBy.is_null())
        .order_by_asc(dtos::ApplicationAssignment::Column::Id);
    if let Some(id) = application_config_id {
        query = query.filter(dtos::ApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(dtos::ApplicationAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(dtos::ApplicationAssignment::Column::GroupId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

// Lists the application assignments that apply to a single device: those
// assigned to the device directly plus those assigned to the device's group
// (if it has one). Used to resolve the full target app set for a device.
pub async fn list_application_assignments_for_device(
    device_id: i32,
    group_id: Option<i32>,
    application_config_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<ApplicationAssignment::Model>, u64), DbErr> {
    let db = db!();

    // (device_id = X) OR (group_id = Y) — a single assignment targets either a
    // device or a group, never both, so OR is the correct combinator here.
    let mut applies_to_device =
        Condition::any().add(dtos::ApplicationAssignment::Column::DeviceId.eq(device_id));
    if let Some(gid) = group_id {
        applies_to_device =
            applies_to_device.add(dtos::ApplicationAssignment::Column::GroupId.eq(gid));
    }

    let mut query = dtos::ApplicationAssignment::Entity::find()
        .filter(dtos::ApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationAssignment::Column::SupersededBy.is_null())
        // Sort by rows with device_id to give them priority over ones with group_id
        .order_by_desc(dtos::ApplicationAssignment::Column::DeviceId)
        .order_by_asc(dtos::ApplicationAssignment::Column::Id)
        .filter(applies_to_device);
    if let Some(id) = application_config_id {
        query = query.filter(dtos::ApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }

    let all = query.all(&db).await?;

    // Resolve application_config_ids to application_ids to remove duplicates that were assigned from a group
    let app_config_ids: Vec<i32> = all.iter().map(|m| m.application_config_id).collect();
    let app_configs = dtos::ApplicationConfig::Entity::find()
        .filter(dtos::ApplicationConfig::Column::Id.is_in(app_config_ids))
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null())
        .all(&db)
        .await?;
    let config_id_to_app_id: std::collections::HashMap<i32, i32> = app_configs
        .into_iter()
        .map(|c| (c.id, c.application_id))
        .collect();

    // Deduplicate by application_id. As rows are ordered by device_id, a assignment with device_id will always win against a assignment with group_id
    let mut seen_application_ids: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let deduplicated: Vec<_> = all
        .into_iter()
        .filter(
            |m| match config_id_to_app_id.get(&m.application_config_id) {
                None => false,
                Some(&app_id) => seen_application_ids.insert(app_id),
            },
        )
        .collect();

    // Manual pagination
    let total_items = deduplicated.len() as u64;
    let start = (page * page_size) as usize;
    let paged = deduplicated
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .map(|m| m.into_api())
        .collect();

    Ok((paged, total_items))
}

pub async fn get_application_assignment(
    id: i32,
) -> Result<Option<ApplicationAssignment::Model>, DbErr> {
    let db = db!();
    Ok(dtos::ApplicationAssignment::Entity::find_by_id(id)
        .filter(dtos::ApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

async fn add_application_assignment(
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let app_assignment = dtos::ApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    log::debug!(
        "Inserted new application config assignment: {:?}",
        new_app_assignment
    );

    Ok(new_app_assignment.into_api())
}

pub async fn update_application_assignment(
    id: i32,
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let db = db!();

    let current = dtos::ApplicationAssignment::Entity::find_by_id(id)
        .filter(dtos::ApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "ApplicationAssignment not found".into(),
        ))?;

    let new_assignment = dtos::ApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };
    let new_assignment = new_assignment.insert(&db).await?;

    let mut old_active: dtos::ApplicationAssignment::ActiveModel = current.into();
    old_active.superseded_by = Set(Some(new_assignment.id));
    old_active.update(&db).await?;

    log::debug!(
        "Updated application assignment via append-only: {:?}",
        new_assignment
    );
    Ok(new_assignment.into_api())
}

pub async fn delete_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::ApplicationAssignment::Entity::find_by_id(id)
        .filter(dtos::ApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(assignment) => {
            let mut active: dtos::ApplicationAssignment::ActiveModel = assignment.into();
            active.deleted_at = Set(Some(chrono::Utc::now()));
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}

pub async fn add_application_assignment_to_device(
    app_config_id: i32,
    device_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, Some(device_id), None).await
}

pub async fn add_application_assignment_to_group(
    app_config_id: i32,
    group_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, None, Some(group_id)).await
}
