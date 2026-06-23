use std::collections::HashMap;

use crate::dtos;
use amos_common::entities::ApplicationConfig;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::db;

// --Application Configs--

pub async fn list_application_configs(
    application_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<ApplicationConfig::Model>, u64), DbErr> {
    let db = db!();
    let mut query =
        dtos::ApplicationConfig::Entity::find().order_by_asc(dtos::ApplicationConfig::Column::Id);
    if let Some(id) = application_id {
        query = query.filter(dtos::ApplicationConfig::Column::ApplicationId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(dtos::ApplicationConfig::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(dtos::ApplicationConfig::Column::GroupId.eq(id));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_application_config(id: i32) -> Result<Option<ApplicationConfig::Model>, DbErr> {
    let db = db!();
    Ok(dtos::ApplicationConfig::Entity::find_by_id(id)
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

/// Resolves the effective application configs for a device: configs
/// assigned directly to the device, plus configs assigned to the device's
/// group. When both exist for the same application, the device-specific
/// config supersedes the group config.
pub async fn list_application_configs_for_device(
    device_id: i32,
) -> Result<Vec<ApplicationConfig::Model>, DbErr> {
    let db = db!();

    let group_id = dtos::Device::Entity::find_by_id(device_id)
        .one(&db)
        .await?
        .and_then(|d| d.group_id);

    let mut query =
        dtos::ApplicationConfig::Entity::find().filter(dtos::ApplicationConfig::Column::DeviceId.eq(device_id));
    if let Some(group_id) = group_id {
        query = dtos::ApplicationConfig::Entity::find().filter(
            dtos::ApplicationConfig::Column::DeviceId
                .eq(device_id)
                .or(dtos::ApplicationConfig::Column::GroupId.eq(group_id)),
        );
    }

    let configs = query
        .order_by_asc(dtos::ApplicationConfig::Column::Id)
        .all(&db)
        .await?;

    let mut by_application: HashMap<i32, dtos::application_config::Model> = HashMap::new();
    for config in configs.iter().filter(|c| c.group_id.is_some()) {
        by_application.insert(config.application_id, config.clone());
    }
    for config in configs.iter().filter(|c| c.device_id == Some(device_id)) {
        by_application.insert(config.application_id, config.clone());
    }

    let mut result: Vec<_> = by_application.into_values().map(|m| m.into_api()).collect();
    result.sort_by_key(|c| c.id);
    Ok(result)
}

pub async fn add_application_config(
    device_id: Option<i32>,
    group_id: Option<i32>,
    application_id: i32,
    image: String,
    config: String,
    version: i32,
) -> Result<ApplicationConfig::Model, DbErr> {
    let app_config = dtos::ApplicationConfig::ActiveModel {
        id: NotSet,
        device_id: Set(device_id),
        group_id: Set(group_id),
        application_id: Set(application_id),
        image: Set(image),
        config: Set(config),
        version: Set(version),
    };

    let db = db!();

    let new_app_config = app_config.insert(&db).await?;
    debug!("Inserted new application config: {:?}", new_app_config);

    Ok(new_app_config.into_api())
}

pub async fn update_application_config(
    id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
    application_id: i32,
    image: String,
    config: String,
    version: i32,
) -> Result<ApplicationConfig::Model, DbErr> {
    let db = db!();
    let app_config = dtos::ApplicationConfig::ActiveModel {
        id: Set(id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        application_id: Set(application_id),
        image: Set(image),
        config: Set(config),
        version: Set(version),
    };
    let updated = app_config.update(&db).await?;
    debug!("Updated application config: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_application_config(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::ApplicationConfig::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
