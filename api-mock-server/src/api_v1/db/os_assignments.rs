use amos_common::entities::OsAssignment;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter};

use super::db;

// --OS Assignments--

pub async fn list_os_assignments(
    os_version_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<Vec<OsAssignment::Model>, DbErr> {
    let db = db!();
    let mut query = OsAssignment::Entity::find();
    if let Some(id) = os_version_id {
        query = query.filter(OsAssignment::Column::OsVersionId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(OsAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(OsAssignment::Column::GroupId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_os_assignment(id: i32) -> Result<Option<OsAssignment::Model>, DbErr> {
    let db = db!();
    OsAssignment::Entity::find_by_id(id).one(&db).await
}

pub async fn add_os_assignment(
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<OsAssignment::Model, DbErr> {
    let os_assignment = OsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_os_assignment = os_assignment.insert(&db).await?;
    debug!(
        "Inserted new OS version assignment: {:?}",
        new_os_assignment
    );

    Ok(new_os_assignment)
}

pub async fn update_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<OsAssignment::Model, DbErr> {
    let db = db!();
    let os_assignment = OsAssignment::ActiveModel {
        id: Set(id),
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };
    let updated_os_assignment = os_assignment.update(&db).await?;
    debug!("Updated OS version assignment: {:?}", updated_os_assignment);
    Ok(updated_os_assignment)
}

pub async fn delete_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = OsAssignment::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
