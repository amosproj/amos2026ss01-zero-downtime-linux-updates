use amos_common::entities::OsVersion;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, PaginatorTrait, QueryOrder};

use super::db;

// --OS Versions--

pub async fn list_os_versions(
    page: u64,
    page_size: u64,
) -> Result<(Vec<OsVersion::Model>, u64), DbErr> {
    let db = db!();
    let query = OsVersion::Entity::find().order_by_asc(OsVersion::Column::Id);
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn get_os_version(id: i32) -> Result<Option<OsVersion::Model>, DbErr> {
    let db = db!();
    OsVersion::Entity::find_by_id(id).one(&db).await
}

pub async fn add_os_version(
    commit_hash: String,
    orchestrator_version: String,
    description: Option<String>,
) -> Result<OsVersion::Model, DbErr> {
    let os_version = OsVersion::ActiveModel {
        id: NotSet,
        commit_hash: Set(commit_hash),
        orchestrator_version: Set(orchestrator_version),
        description: Set(description),
    };

    let db = db!();

    let new_os_version = os_version.insert(&db).await?;
    debug!("Inserted new OS version: {:?}", new_os_version);

    Ok(new_os_version)
}

pub async fn update_os_version(
    id: i32,
    commit_hash: String,
    orchestrator_version: String,
    description: Option<String>,
) -> Result<OsVersion::Model, DbErr> {
    let db = db!();
    let os_version = OsVersion::ActiveModel {
        id: Set(id),
        commit_hash: Set(commit_hash),
        orchestrator_version: Set(orchestrator_version),
        description: Set(description),
    };
    let updated_os_version = os_version.update(&db).await?;
    debug!("Updated OS version: {:?}", updated_os_version);
    Ok(updated_os_version)
}

pub async fn delete_os_version(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = OsVersion::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
