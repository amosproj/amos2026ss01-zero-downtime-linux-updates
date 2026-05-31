use amos_common::entities::ReportedOsAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
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
    let mut query =
        ReportedOsAssignment::Entity::find().order_by_asc(ReportedOsAssignment::Column::Id);
    if let Some(id) = device_id {
        query = query.filter(ReportedOsAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = os_version_id {
        query = query.filter(ReportedOsAssignment::Column::OsVersionId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn get_reported_os_assignment(
    id: i32,
) -> Result<Option<ReportedOsAssignment::Model>, DbErr> {
    let db = db!();
    ReportedOsAssignment::Entity::find_by_id(id).one(&db).await
}

pub async fn add_reported_os_assignment(
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let os_assignment = ReportedOsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };

    let db = db!();

    let new_os_assignment = os_assignment.insert(&db).await?;
    debug!(
        "Inserted new reported OS version assignment: {:?}",
        new_os_assignment
    );
    Ok(new_os_assignment)
}

#[allow(dead_code)]
pub async fn update_reported_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let db = db!();
    let os_assignment = ReportedOsAssignment::ActiveModel {
        id: Set(id),
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };
    let updated_os_assignment = os_assignment.update(&db).await?;
    debug!(
        "Updated reported OS version assignment: {:?}",
        updated_os_assignment
    );
    Ok(updated_os_assignment)
}

pub async fn delete_reported_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ReportedOsAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
