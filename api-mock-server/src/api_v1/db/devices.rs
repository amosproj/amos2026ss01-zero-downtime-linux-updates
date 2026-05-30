use amos_common::entities::Device;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, ExprTrait};
use sea_orm::sea_query::Expr;

use super::db;

// --Devices--

pub async fn list_devices(
    group_id: Option<i32>,
    tenant_id: Option<i32>,
    uuid_filter: Option<String>,
    hostname_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Device::Model>, u64), DbErr> {
    let db = db!();
    let mut query = Device::Entity::find().order_by_asc(Device::Column::Id);
    if let Some(id) = group_id {
        query = query.filter(Device::Column::GroupId.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(Device::Column::TenantId.eq(id));
    }
    if let Some(uuid) = uuid_filter {
        query = query.filter(Expr::col(Device::Column::Uuid).like(format!("%{}%", uuid)));
    }
    if let Some(hostname) = hostname_filter {
        query = query.filter(Expr::col(Device::Column::Hostname).like(format!("%{}%", hostname)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
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
