use crate::dtos;
use amos_common::entities::ApplicationConfig;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Application Configs--

pub async fn list_application_configs(
    application_id: Option<i32>,
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

pub async fn add_application_config(
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let app_config = dtos::ApplicationConfig::ActiveModel {
        id: NotSet,
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_app_config = app_config.insert(&db).await?;
    debug!("Inserted new application config: {:?}", new_app_config);

    Ok(new_app_config.into_api())
}

pub async fn update_application_config(
    id: i32,
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let db = db!();

    let current = dtos::ApplicationConfig::Entity::find_by_id(id)
        .filter(dtos::ApplicationConfig::Column::DeletedAt.is_null())
        .filter(dtos::ApplicationConfig::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("ApplicationConfig not found".into()))?;

    let new_config = dtos::ApplicationConfig::ActiveModel {
        id: NotSet,
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };
    let new_config = new_config.insert(&db).await?;

    let mut old_active: dtos::ApplicationConfig::ActiveModel = current.into();
    old_active.superseded_by = Set(Some(new_config.id));
    old_active.update(&db).await?;

    debug!(
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
