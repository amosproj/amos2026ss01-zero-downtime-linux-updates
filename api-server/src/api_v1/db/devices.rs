use crate::dtos;
use amos_common::entities::Device;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::db;

// --Devices--

pub async fn list_devices(
    group_id: Option<i32>,
    tenant_id: Option<i32>,
    uuid_filter: Option<String>,
    serial_number_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Device::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::Device::Entity::find().order_by_asc(dtos::Device::Column::Id);
    if let Some(id) = group_id {
        query = query.filter(dtos::Device::Column::GroupId.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(dtos::Device::Column::TenantId.eq(id));
    }
    if let Some(uuid) = uuid_filter {
        query = query.filter(Expr::col(dtos::Device::Column::Uuid).like(format!("%{}%", uuid)));
    }
    if let Some(serial_number) = serial_number_filter {
        query = query.filter(
            Expr::col(dtos::Device::Column::SerialNumber).like(format!("%{}%", serial_number)),
        );
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_device(id: i32) -> Result<Option<Device::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Device::Entity::find_by_id(id)
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

/// Returns the Device for a given uuid, empty Option if the uuid does not exist
pub async fn get_device_by_uuid(uuid: String) -> Result<Option<Device::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Device::Entity::find()
        .filter(dtos::Device::Column::Uuid.eq(uuid))
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_device(
    uuid: String,
    public_key: Option<String>,
    serial_number: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let device = dtos::Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        public_key: Set(public_key),
        serial_number: Set(serial_number),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_device = device.insert(&db).await?;
    log::debug!("Inserted device: {:?}", new_device);

    Ok(new_device.into_api())
}

pub async fn update_device(
    id: i32,
    uuid: String,
    public_key: Option<String>,
    serial_number: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let db = db!();
    let device = dtos::Device::Entity::find_by_id(id)
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Device not found".into()))?;
    let mut device: dtos::Device::ActiveModel = device.into();
    device.uuid = Set(uuid);
    device.public_key = Set(public_key);
    device.serial_number = Set(serial_number);
    device.tenant_id = Set(tenant_id);
    device.group_id = Set(group_id);
    let updated_device = device.update(&db).await?;
    log::debug!("Updated device: {:?}", updated_device);
    Ok(updated_device.into_api())
}

pub async fn patch_device(
    id: i32,
    uuid: Option<String>,
    public_key: Option<String>,
    serial_number: Option<String>,
    tenant_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let db = db!();
    let device = dtos::Device::Entity::find_by_id(id)
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Device not found".into()))?;
    let mut device: dtos::Device::ActiveModel = device.into();

    // Conditionally update fields
    if let Some(uuid) = uuid {
        device.uuid = Set(uuid);
    }
    if let Some(public_key) = public_key {
        device.public_key = Set(Some(public_key));
    }
    if let Some(serial_number) = serial_number {
        device.serial_number = Set(serial_number);
    }
    if let Some(tenant_id) = tenant_id {
        device.tenant_id = Set(tenant_id);
    }
    if let Some(group_id) = group_id {
        device.group_id = Set(Some(group_id));
    }

    let updated_device = device.update(&db).await?;
    log::debug!("Patched device: {:?}", updated_device);
    Ok(updated_device.into_api())
}

pub async fn delete_device(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::Device::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
