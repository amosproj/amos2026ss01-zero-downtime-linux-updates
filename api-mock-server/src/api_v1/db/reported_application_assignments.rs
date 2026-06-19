use crate::dtos;
use amos_common::entities::ReportedApplicationAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Reported Application Assignments--

pub async fn list_reported_application_assignments(
    device_id: Option<i32>,
    application_config_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<ReportedApplicationAssignment::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::ReportedApplicationAssignment::Entity::find()
        .filter(dtos::ReportedApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedApplicationAssignment::Column::SupersededBy.is_null())
        .order_by_asc(dtos::ReportedApplicationAssignment::Column::Id);
    if let Some(id) = device_id {
        query = query.filter(dtos::ReportedApplicationAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = application_config_id {
        query =
            query.filter(dtos::ReportedApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_reported_application_assignment(
    id: i32,
) -> Result<Option<ReportedApplicationAssignment::Model>, DbErr> {
    let db = db!();
    Ok(dtos::ReportedApplicationAssignment::Entity::find_by_id(id)
        .filter(dtos::ReportedApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedApplicationAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_reported_application_assignment(
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let app_assignment = dtos::ReportedApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: NotSet,
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    debug!(
        "Inserted new reported application assignment: {:?}",
        new_app_assignment
    );
    Ok(new_app_assignment.into_api())
}

#[allow(dead_code)]
pub async fn update_reported_application_assignment(
    id: i32,
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let db = db!();

    let current = dtos::ReportedApplicationAssignment::Entity::find_by_id(id)
        .filter(dtos::ReportedApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedApplicationAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "ReportedApplicationAssignment not found".into(),
        ))?;

    let active = dtos::ReportedApplicationAssignment::ActiveModel {
        id: Set(current.id),
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: Set(current.updated_at),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(current.superseded_by),
    };
    let updated = active.update(&db).await?;
    debug!("Updated reported application assignment: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_reported_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::ReportedApplicationAssignment::Entity::find_by_id(id)
        .filter(dtos::ReportedApplicationAssignment::Column::DeletedAt.is_null())
        .filter(dtos::ReportedApplicationAssignment::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(assignment) => {
            let active = dtos::ReportedApplicationAssignment::ActiveModel {
                id: Set(assignment.id),
                application_config_id: Set(assignment.application_config_id),
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
