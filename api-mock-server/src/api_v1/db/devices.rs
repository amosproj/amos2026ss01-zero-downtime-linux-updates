use crate::dtos;
use amos_common::entities::Device;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::sea_query::prelude::chrono;
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
    hostname_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Device::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::Device::Entity::find()
        .filter(dtos::Device::Column::DeletedAt.is_null())
        .filter(dtos::Device::Column::SupersededBy.is_null())
        .order_by_asc(dtos::Device::Column::Id);
    if let Some(id) = group_id {
        query = query.filter(dtos::Device::Column::GroupId.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(dtos::Device::Column::TenantId.eq(id));
    }
    if let Some(uuid) = uuid_filter {
        query = query.filter(Expr::col(dtos::Device::Column::Uuid).like(format!("%{}%", uuid)));
    }
    if let Some(hostname) = hostname_filter {
        query =
            query.filter(Expr::col(dtos::Device::Column::Hostname).like(format!("%{}%", hostname)));
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
        .filter(dtos::Device::Column::DeletedAt.is_null())
        .filter(dtos::Device::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

/// Returns the Device for a given uuid, empty Option if the uuid does not exist
pub async fn get_device_by_uuid(uuid: String) -> Result<Option<Device::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Device::Entity::find()
        .filter(dtos::Device::Column::Uuid.eq(uuid))
        .filter(dtos::Device::Column::DeletedAt.is_null())
        .filter(dtos::Device::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_device(
    uuid: String,
    public_key: Option<String>,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let device = dtos::Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        public_key: Set(public_key),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_device = device.insert(&db).await?;
    debug!("Inserted device: {:?}", new_device);

    Ok(new_device.into_api())
}

pub async fn update_device(
    id: i32,
    uuid: String,
    public_key: Option<String>,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let db = db!();

    let current = dtos::Device::Entity::find_by_id(id)
        .filter(dtos::Device::Column::DeletedAt.is_null())
        .filter(dtos::Device::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Device not found".into()))?;

    let active = dtos::Device::ActiveModel {
        id: Set(current.id),
        uuid: Set(uuid),
        public_key: Set(public_key),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(current.superseded_by),
    };
    let updated = active.update(&db).await?;
    debug!("Updated device: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_device(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::Device::Entity::find_by_id(id)
        .filter(dtos::Device::Column::DeletedAt.is_null())
        .filter(dtos::Device::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(device) => {
            let active = dtos::Device::ActiveModel {
                id: Set(device.id),
                uuid: Set(device.uuid),
                public_key: Set(device.public_key),
                hostname: Set(device.hostname),
                tenant_id: Set(device.tenant_id),
                group_id: Set(device.group_id),
                deleted_at: Set(Some(chrono::Utc::now())),
                superseded_by: Set(device.superseded_by),
            };
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
