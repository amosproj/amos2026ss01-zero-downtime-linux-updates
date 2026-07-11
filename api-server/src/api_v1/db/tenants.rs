use crate::dtos;
use amos_common::entities::Tenant;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Tenants--

pub async fn list_tenants(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Tenant::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::Tenant::Entity::find().order_by_asc(dtos::Tenant::Column::Id);
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
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_tenant(name: String, description: Option<String>) -> Result<Tenant::Model, DbErr> {
    let tenant = dtos::Tenant::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_tenant = tenant.insert(&db).await?;
    log::debug!("Inserted new tenant: {:?}", new_tenant);

    Ok(new_tenant.into_api())
}

pub async fn update_tenant(
    id: i32,
    name: String,
    description: Option<String>,
) -> Result<Tenant::Model, DbErr> {
    let db = db!();
    let tenant = dtos::Tenant::Entity::find_by_id(id)
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Tenant not found".into()))?;
    let mut tenant: dtos::Tenant::ActiveModel = tenant.into();
    tenant.name = Set(name);
    tenant.description = Set(description);
    let updated_tenant = tenant.update(&db).await?;
    log::debug!("Updated tenant: {:?}", updated_tenant);
    Ok(updated_tenant.into_api())
}

pub async fn delete_tenant(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::Tenant::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
