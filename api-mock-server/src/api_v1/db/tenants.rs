use amos_common::entities::Tenant;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, ExprTrait};
use sea_orm::sea_query::Expr;

use super::db;

// --Tenants--

pub async fn list_tenants(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Tenant::Model>, u64), DbErr> {
    let db = db!();
    let mut query = Tenant::Entity::find().order_by_asc(Tenant::Column::Id);
    if let Some(name) = name_filter {
        query = query.filter(Expr::col(Tenant::Column::Name).like(format!("%{}%", name)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn get_tenant(id: i32) -> Result<Option<Tenant::Model>, DbErr> {
    let db = db!();
    Tenant::Entity::find_by_id(id).one(&db).await
}

pub async fn add_tenant(name: String, description: Option<String>) -> Result<Tenant::Model, DbErr> {
    let tenant = Tenant::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_tenant = tenant.insert(&db).await?;
    debug!("Inserted new tenant: {:?}", new_tenant);

    Ok(new_tenant)
}

pub async fn update_tenant(
    id: i32,
    name: String,
    description: Option<String>,
) -> Result<Tenant::Model, DbErr> {
    let db = db!();
    let tenant = Tenant::ActiveModel {
        id: Set(id),
        name: Set(name),
        description: Set(description),
    };
    let updated_tenant = tenant.update(&db).await?;
    debug!("Updated tenant: {:?}", updated_tenant);
    Ok(updated_tenant)
}

pub async fn delete_tenant(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Tenant::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
