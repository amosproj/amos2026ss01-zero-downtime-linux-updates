use crate::dtos;
use amos_common::entities::OsVersion;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --OS Versions--

pub async fn list_os_versions(
    page: u64,
    page_size: u64,
) -> Result<(Vec<OsVersion::Model>, u64), DbErr> {
    let db = db!();
    let query = dtos::OsVersion::Entity::find()
        .filter(dtos::OsVersion::Column::DeletedAt.is_null())
        .filter(dtos::OsVersion::Column::SupersededBy.is_null())
        .order_by_asc(dtos::OsVersion::Column::Id);
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_os_version(id: i32) -> Result<Option<OsVersion::Model>, DbErr> {
    let db = db!();
    Ok(dtos::OsVersion::Entity::find_by_id(id)
        .filter(dtos::OsVersion::Column::DeletedAt.is_null())
        .filter(dtos::OsVersion::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_os_version(
    commit_hash: String,
    orchestrator_version: String,
    description: Option<String>,
) -> Result<OsVersion::Model, DbErr> {
    let os_version = dtos::OsVersion::ActiveModel {
        id: NotSet,
        commit_hash: Set(commit_hash),
        orchestrator_version: Set(orchestrator_version),
        description: Set(description),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_os_version = os_version.insert(&db).await?;
    log::trace!("Inserted new OS version: {:?}", new_os_version);

    Ok(new_os_version.into_api())
}

pub async fn update_os_version(
    id: i32,
    commit_hash: String,
    orchestrator_version: String,
    description: Option<String>,
) -> Result<OsVersion::Model, DbErr> {
    let db = db!();

    let current = dtos::OsVersion::Entity::find_by_id(id)
        .filter(dtos::OsVersion::Column::DeletedAt.is_null())
        .filter(dtos::OsVersion::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("OsVersion not found".into()))?;

    let new_version = dtos::OsVersion::ActiveModel {
        id: NotSet,
        commit_hash: Set(commit_hash),
        orchestrator_version: Set(orchestrator_version),
        description: Set(description),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };
    let new_version = new_version.insert(&db).await?;

    let mut old_active: dtos::OsVersion::ActiveModel = current.into();
    old_active.superseded_by = Set(Some(new_version.id));
    old_active.update(&db).await?;

    log::trace!("Updated OS version via append-only: {:?}", new_version);
    Ok(new_version.into_api())
}

pub async fn delete_os_version(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::OsVersion::Entity::find_by_id(id)
        .filter(dtos::OsVersion::Column::DeletedAt.is_null())
        .filter(dtos::OsVersion::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(os_version) => {
            let mut active: dtos::OsVersion::ActiveModel = os_version.into();
            active.deleted_at = Set(Some(chrono::Utc::now()));
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
