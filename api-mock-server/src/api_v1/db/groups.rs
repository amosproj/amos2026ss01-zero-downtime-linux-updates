use amos_common::entities::Group;
use crate::dtos;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait};

use super::db;

// --Groups--

pub async fn list_groups() -> Result<Vec<Group::Model>, DbErr> {
    let db = db!();
    dtos::Group::Entity::find()
        .all(&db)
        .await
        .map(|v| v.into_iter().map(|m| m.into_api()).collect())
}

pub async fn get_group(id: i32) -> Result<Option<Group::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Group::Entity::find_by_id(id)
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_group(name: String) -> Result<Group::Model, DbErr> {
    let group = dtos::Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
    };

    let db = db!();

    let new_group = group.insert(&db).await?;
    debug!("Inserted group: {:?}", new_group);

    Ok(new_group.into_api())
}

pub async fn update_group(id: i32, name: String) -> Result<Group::Model, DbErr> {
    let db = db!();
    let group = dtos::Group::ActiveModel {
        id: Set(id),
        name: Set(name),
    };
    let updated_group = group.update(&db).await?;
    debug!("Updated group: {:?}", updated_group);
    Ok(updated_group.into_api())
}

pub async fn delete_group(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::Group::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
