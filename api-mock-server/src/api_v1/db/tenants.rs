use amos_common::entities::Tenant;
use crate::dtos;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait};

use super::db;

// --Tenants--

pub async fn list_tenants() -> Result<Vec<Tenant::Model>, DbErr> {
    let db = db!();
    dtos::Tenant::Entity::find()
        .all(&db)
        .await
        .map(|v| v.into_iter().map(|m| m.into_api()).collect())
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
    debug!("Inserted new tenant: {:?}", new_tenant);

    Ok(new_tenant.into_api())
}

pub async fn update_tenant(
    id: i32,
    name: String,
    description: Option<String>,
) -> Result<Tenant::Model, DbErr> {
    let db = db!();
    let tenant = dtos::Tenant::ActiveModel {
        id: Set(id),
        name: Set(name),
        description: Set(description),
    };
    let updated_tenant = tenant.update(&db).await?;
    debug!("Updated tenant: {:?}", updated_tenant);
    Ok(updated_tenant.into_api())
}

pub async fn delete_tenant(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::Tenant::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
