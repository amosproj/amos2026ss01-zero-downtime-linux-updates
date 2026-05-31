use amos_common::entities::ReportedApplicationAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
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
    let mut query = ReportedApplicationAssignment::Entity::find()
        .order_by_asc(ReportedApplicationAssignment::Column::Id);
    if let Some(id) = device_id {
        query = query.filter(ReportedApplicationAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = application_config_id {
        query = query.filter(ReportedApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn get_reported_application_assignment(
    id: i32,
) -> Result<Option<ReportedApplicationAssignment::Model>, DbErr> {
    let db = db!();
    ReportedApplicationAssignment::Entity::find_by_id(id)
        .one(&db)
        .await
}

#[allow(dead_code)]
pub async fn add_reported_application_assignment(
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let app_assignment = ReportedApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    debug!(
        "Inserted new reported application assignment: {:?}",
        new_app_assignment
    );
    Ok(new_app_assignment)
}

#[allow(dead_code)]
pub async fn update_reported_application_assignment(
    id: i32,
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let db = db!();
    let app_assignment = ReportedApplicationAssignment::ActiveModel {
        id: Set(id),
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };
    let updated_group = app_assignment.update(&db).await?;
    debug!(
        "Updated reported application assignment: {:?}",
        updated_group
    );
    Ok(updated_group)
}

pub async fn delete_reported_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ReportedApplicationAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
