use crate::dtos;
use amos_common::entities::Tenant;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::db;

// --Tenants--

pub async fn list_tenants(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Tenant::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::Tenant::Entity::find()
        .filter(dtos::Tenant::Column::DeletedAt.is_null())
        .filter(dtos::Tenant::Column::SupersededBy.is_null())
        .order_by_asc(dtos::Tenant::Column::Id);
    if let Some(name) = name_filter {
        query = query.filter(Expr::col(dtos::Tenant::Column::Name).like(format!("%{}%", name)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_tenant(id: i32) -> Result<Option<Tenant::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Tenant::Entity::find_by_id(id)
        .filter(dtos::Tenant::Column::DeletedAt.is_null())
        .filter(dtos::Tenant::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_tenant(name: String, description: Option<String>) -> Result<Tenant::Model, DbErr> {
    let tenant = dtos::Tenant::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_tenant = tenant.insert(&db).await?;
    debug!("Inserted new tenant: {:?}", new_tenant);

    Ok(new_tenant.into_api())
}

pub async fn update_tenant(
    id: i32,
    name: String,
    description: Option<String>,
) -> Result<Tenant::Model, DbErr> {
    let db = db!();

    let current = dtos::Tenant::Entity::find_by_id(id)
        .filter(dtos::Tenant::Column::DeletedAt.is_null())
        .filter(dtos::Tenant::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Tenant not found".into()))?;

    let active = dtos::Tenant::ActiveModel {
        id: Set(current.id),
        name: Set(name),
        description: Set(description),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(current.superseded_by),
    };
    let updated = active.update(&db).await?;
    debug!("Updated tenant: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_tenant(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::Tenant::Entity::find_by_id(id)
        .filter(dtos::Tenant::Column::DeletedAt.is_null())
        .filter(dtos::Tenant::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(tenant) => {
            let active = dtos::Tenant::ActiveModel {
                id: Set(tenant.id),
                name: Set(tenant.name),
                description: Set(tenant.description),
                deleted_at: Set(Some(chrono::Utc::now())),
                superseded_by: Set(tenant.superseded_by),
            };
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
