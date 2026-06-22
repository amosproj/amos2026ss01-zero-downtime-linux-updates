use crate::dtos;
use amos_common::entities::DeviceApplicationConfig;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Device Application Configs--

pub async fn list_device_application_configs(
    device_id: Option<i32>,
    application_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<DeviceApplicationConfig::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::DeviceApplicationConfig::Entity::find()
        .order_by_asc(dtos::DeviceApplicationConfig::Column::Id);
    if let Some(id) = device_id {
        query = query.filter(dtos::DeviceApplicationConfig::Column::DeviceId.eq(id));
    }
    if let Some(id) = application_id {
        query = query.filter(dtos::DeviceApplicationConfig::Column::ApplicationId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_device_application_config(
    id: i32,
) -> Result<Option<DeviceApplicationConfig::Model>, DbErr> {
    let db = db!();
    Ok(dtos::DeviceApplicationConfig::Entity::find_by_id(id)
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_device_application_config(
    device_id: i32,
    application_id: i32,
    config: String,
    version: i32,
) -> Result<DeviceApplicationConfig::Model, DbErr> {
    let device_app_config = dtos::DeviceApplicationConfig::ActiveModel {
        id: NotSet,
        device_id: Set(device_id),
        application_id: Set(application_id),
        config: Set(config),
        version: Set(version),
    };

    let db = db!();

    let new_device_app_config = device_app_config.insert(&db).await?;
    debug!(
        "Inserted new device application config: {:?}",
        new_device_app_config
    );

    Ok(new_device_app_config.into_api())
}

pub async fn update_device_application_config(
    id: i32,
    device_id: i32,
    application_id: i32,
    config: String,
    version: i32,
) -> Result<DeviceApplicationConfig::Model, DbErr> {
    let db = db!();
    let device_app_config = dtos::DeviceApplicationConfig::ActiveModel {
        id: Set(id),
        device_id: Set(device_id),
        application_id: Set(application_id),
        config: Set(config),
        version: Set(version),
    };
    let updated = device_app_config.update(&db).await?;
    debug!("Updated device application config: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_device_application_config(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::DeviceApplicationConfig::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
