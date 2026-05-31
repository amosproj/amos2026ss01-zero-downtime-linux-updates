use crate::dtos;
use amos_common::entities::ApplicationAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
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
    Ok((data.into_iter().map(|m| m.into_api()).collect(), total_items))
}

pub async fn get_application_assignment(
    id: i32,
) -> Result<Option<ApplicationAssignment::Model>, DbErr> {
    let db = db!();
    Ok(dtos::ApplicationAssignment::Entity::find_by_id(id)
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
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    debug!(
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
    let app_assignment = dtos::ApplicationAssignment::ActiveModel {
        id: Set(id),
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };
    let updated_group = app_assignment.update(&db).await?;
    debug!("Updated application assignment: {:?}", updated_group);
    Ok(updated_group.into_api())
}

pub async fn delete_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::ApplicationAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
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
