use crate::dtos;
use amos_common::entities::ReportedApplicationAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
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
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_reported_application_assignment(
    application_config_id: i32,
    device_id: i32,
) -> Result<(), DbErr> {
    let app_assignment = dtos::ReportedApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: Set(chrono::Utc::now()),
    };

    let db = db!();

    dtos::ReportedApplicationAssignment::Entity::insert(app_assignment)
        .on_conflict(
            OnConflict::columns([
                dtos::ReportedApplicationAssignment::Column::DeviceId,
                dtos::ReportedApplicationAssignment::Column::ApplicationConfigId,
            ])
            .update_column(dtos::ReportedApplicationAssignment::Column::UpdatedAt)
            .to_owned(),
        )
        .exec(&db)
        .await?;

    debug!(
        "Upserted new reported application assignment: device={} config={}",
        device_id, application_config_id
    );
    Ok(())
}

#[allow(dead_code)]
pub async fn update_reported_application_assignment(
    id: i32,
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let db = db!();
    let app_assignment = dtos::ReportedApplicationAssignment::Entity::find_by_id(id)
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound(
            "ReportedApplicationAssignment not found".into(),
        ))?;
    let mut app_assignment: dtos::ReportedApplicationAssignment::ActiveModel =
        app_assignment.into();
    app_assignment.application_config_id = Set(application_config_id);
    app_assignment.device_id = Set(device_id);
    let updated = app_assignment.update(&db).await?;
    debug!("Updated reported application assignment: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_reported_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::ReportedApplicationAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
