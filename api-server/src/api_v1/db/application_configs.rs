use std::collections::HashMap;

use crate::dtos;
use amos_common::entities::{ApplicationConfig, ContainerConfigV1};
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::TransactionTrait;
use sea_orm::sea_query::prelude::chrono;
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
    let mut query = dtos::ApplicationConfig::Entity::find()
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null())
        .order_by_asc(dtos::ApplicationConfig::Column::Id);
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
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null())
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

    let mut query = dtos::ApplicationConfig::Entity::find()
        .filter(dtos::ApplicationConfig::Column::DeviceId.eq(device_id));
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
    config: Option<ContainerConfigV1>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let db = db!();
    let txn = db.begin().await?;

    let mut old_query = dtos::ApplicationConfig::Entity::find()
        .filter(dtos::ApplicationConfig::Column::ApplicationId.eq(application_id))
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null());

    if let Some(did) = device_id {
        old_query = old_query.filter(dtos::ApplicationConfig::Column::DeviceId.eq(did));
    } else if let Some(gid) = group_id {
        old_query = old_query.filter(dtos::ApplicationConfig::Column::GroupId.eq(gid));
    }

    let existing_configs = old_query.all(&txn).await?;

    let next_config_version = existing_configs
        .first()
        .map(|c| c.config_version)
        .unwrap_or(1);
    let next_version = existing_configs
        .iter()
        .map(|c| c.version)
        .max()
        .unwrap_or(0)
        + 1;

    let config_json = config
        .map(|c| serde_json::to_string(&c))
        .transpose()
        .map_err(|e| DbErr::Custom(format!("Failed to serialize config: {e}")))?;

    let app_config = dtos::ApplicationConfig::ActiveModel {
        id: NotSet,
        device_id: Set(device_id),
        group_id: Set(group_id),
        application_id: Set(application_id),
        image: Set(image),
        config_version: Set(next_config_version),
        config: Set(config_json),
        version: Set(next_version),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let new_app_config = app_config.insert(&txn).await?;
    log::trace!("Inserted new application config: {:?}", new_app_config);

    for old in existing_configs {
        let old_id = old.id;
        let mut old_active: dtos::ApplicationConfig::ActiveModel = old.into();
        old_active.superseded_by = Set(Some(new_app_config.id));
        old_active.update(&txn).await?;
        log::trace!(
            "Superseded old App config {} with {}",
            old_id, new_app_config.id
        );
    }

    txn.commit().await?;

    Ok(new_app_config.into_api())
}

pub async fn update_application_config(
    id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
    application_id: i32,
    image: String,
    config: Option<ContainerConfigV1>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let db = db!();

    let current = dtos::ApplicationConfig::Entity::find_by_id(id)
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("ApplicationConfig not found".into()))?;

    let config_json = config
        .map(|c| serde_json::to_string(&c))
        .transpose()
        .map_err(|e| DbErr::Custom(format!("Failed to serialize config: {e}")))?;

    let new_config = dtos::ApplicationConfig::ActiveModel {
        id: NotSet,
        application_id: Set(application_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
        image: Set(image),
        config_version: Set(current.config_version),
        config: Set(config_json),
        version: Set(current.version + 1),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };
    let new_config = new_config.insert(&db).await?;

    let mut old_active: dtos::ApplicationConfig::ActiveModel = current.into();
    old_active.superseded_by = Set(Some(new_config.id));
    old_active.update(&db).await?;

    log::trace!(
        "Updated application config via append-only: {:?}",
        new_config
    );
    Ok(new_config.into_api())
}

pub async fn delete_application_config(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::ApplicationConfig::Entity::find_by_id(id)
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(config) => {
            let mut active: dtos::ApplicationConfig::ActiveModel = config.into();
            active.deleted_at = Set(Some(chrono::Utc::now()));
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
