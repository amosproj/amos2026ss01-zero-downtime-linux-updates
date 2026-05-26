use amos_common::entities::Device;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter};

use super::db;

// --Devices--

pub async fn list_devices(
    group_id: Option<i32>,
    tenant_id: Option<i32>,
) -> Result<Vec<Device::Model>, DbErr> {
    let db = db!();
    let mut query = Device::Entity::find();
    if let Some(id) = group_id {
        query = query.filter(Device::Column::GroupId.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(Device::Column::TenantId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_device(id: i32) -> Result<Option<Device::Model>, DbErr> {
    let db = db!();
    Device::Entity::find_by_id(id).one(&db).await
}

/// Returns the Device for a given uuid, empty Option if the uuid does not exist
pub async fn get_device_by_uuid(uuid: String) -> Result<Option<Device::Model>, DbErr> {
    let db = db!();

    Device::Entity::find()
        .filter(Device::Column::Uuid.eq(uuid))
        .one(&db)
        .await
}

pub async fn add_device(
    uuid: String,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let device = Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_device = device.insert(&db).await?;
    debug!("Inserted device: {:?}", new_device);

    Ok(new_device)
}

pub async fn update_device(
    id: i32,
    uuid: String,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let db = db!();
    let device = Device::ActiveModel {
        id: Set(id),
        uuid: Set(uuid),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
    };
    let updated_device = device.update(&db).await?;
    debug!("Updated device: {:?}", updated_device);
    Ok(updated_device)
}

pub async fn delete_device(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Device::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
