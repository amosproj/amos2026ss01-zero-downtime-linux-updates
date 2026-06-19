use crate::dtos;
use amos_common::entities::Group;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::db;

// --Groups--

pub async fn list_groups(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Group::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::Group::Entity::find()
        .filter(dtos::Group::Column::DeletedAt.is_null())
        .filter(dtos::Group::Column::SupersededBy.is_null())
        .order_by_asc(dtos::Group::Column::Id);
    if let Some(name) = name_filter {
        query = query.filter(Expr::col(dtos::Group::Column::Name).like(format!("%{}%", name)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_group(id: i32) -> Result<Option<Group::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Group::Entity::find_by_id(id)
        .filter(dtos::Group::Column::DeletedAt.is_null())
        .filter(dtos::Group::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_group(name: String) -> Result<Group::Model, DbErr> {
    let group = dtos::Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_group = group.insert(&db).await?;
    debug!("Inserted group: {:?}", new_group);

    Ok(new_group.into_api())
}

pub async fn update_group(id: i32, name: String) -> Result<Group::Model, DbErr> {
    let db = db!();

    let current = dtos::Group::Entity::find_by_id(id)
        .filter(dtos::Group::Column::DeletedAt.is_null())
        .filter(dtos::Group::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Group not found".into()))?;

    let active = dtos::Group::ActiveModel {
        id: Set(current.id),
        name: Set(name),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(current.superseded_by),
    };
    let updated = active.update(&db).await?;
    debug!("Updated group: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_group(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::Group::Entity::find_by_id(id)
        .filter(dtos::Group::Column::DeletedAt.is_null())
        .filter(dtos::Group::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(group) => {
            let active = dtos::Group::ActiveModel {
                id: Set(group.id),
                name: Set(group.name),
                deleted_at: Set(Some(chrono::Utc::now())),
                superseded_by: Set(group.superseded_by),
            };
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
