use amos_common::entities::ApplicationAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter};

use super::db;

// --Application Assignments--

pub async fn list_application_assignments(
    application_config_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<Vec<ApplicationAssignment::Model>, DbErr> {
    let db = db!();
    let mut query = ApplicationAssignment::Entity::find();
    if let Some(id) = application_config_id {
        query = query.filter(ApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(ApplicationAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(ApplicationAssignment::Column::GroupId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_application_assignment(
    id: i32,
) -> Result<Option<ApplicationAssignment::Model>, DbErr> {
    let db = db!();
    ApplicationAssignment::Entity::find_by_id(id).one(&db).await
}

pub async fn add_application_assignment(
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let app_assignment = ApplicationAssignment::ActiveModel {
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

    Ok(new_app_assignment)
}

pub async fn update_application_assignment(
    id: i32,
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let db = db!();
    let app_assignment = ApplicationAssignment::ActiveModel {
        id: Set(id),
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };
    let updated_group = app_assignment.update(&db).await?;
    debug!("Updated application assignment: {:?}", updated_group);
    Ok(updated_group)
}

pub async fn delete_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ApplicationAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}

#[allow(dead_code)]
pub async fn add_application_assignment_to_device(
    app_config_id: i32,
    device_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, Some(device_id), None).await
}

#[allow(dead_code)]
pub async fn add_application_assignment_to_group(
    app_config_id: i32,
    group_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, None, Some(group_id)).await
}
