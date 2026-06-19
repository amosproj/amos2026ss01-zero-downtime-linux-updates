use crate::dtos;
use amos_common::entities::Application;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::sea_query::prelude::chrono;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use super::db;

// --Applications--

pub async fn list_applications(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Application::Model>, u64), DbErr> {
    let db = db!();
    let mut query = dtos::Application::Entity::find()
        .filter(dtos::Application::Column::DeletedAt.is_null())
        .filter(dtos::Application::Column::SupersededBy.is_null())
        .order_by_asc(dtos::Application::Column::Id);
    if let Some(name) = name_filter {
        query =
            query.filter(Expr::col(dtos::Application::Column::Name).like(format!("%{}%", name)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn get_application(id: i32) -> Result<Option<Application::Model>, DbErr> {
    let db = db!();
    Ok(dtos::Application::Entity::find_by_id(id)
        .filter(dtos::Application::Column::DeletedAt.is_null())
        .filter(dtos::Application::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .map(|m| m.into_api()))
}

pub async fn add_application(
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let app = dtos::Application::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
        deleted_at: NotSet,
        superseded_by: NotSet,
    };

    let db = db!();

    let new_app = app.insert(&db).await?;
    debug!("Inserted new application: {:?}", new_app);

    Ok(new_app.into_api())
}

pub async fn update_application(
    id: i32,
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let db = db!();

    let current = dtos::Application::Entity::find_by_id(id)
        .filter(dtos::Application::Column::DeletedAt.is_null())
        .filter(dtos::Application::Column::SupersededBy.is_null())
        .one(&db)
        .await?
        .ok_or(DbErr::RecordNotFound("Application not found".into()))?;

    let active = dtos::Application::ActiveModel {
        id: Set(current.id),
        name: Set(name),
        description: Set(description),
        deleted_at: Set(current.deleted_at),
        superseded_by: Set(current.superseded_by),
    };
    let updated = active.update(&db).await?;
    debug!("Updated application: {:?}", updated);
    Ok(updated.into_api())
}

pub async fn delete_application(id: i32) -> Result<u64, DbErr> {
    let db = db!();

    let current = dtos::Application::Entity::find_by_id(id)
        .filter(dtos::Application::Column::DeletedAt.is_null())
        .filter(dtos::Application::Column::SupersededBy.is_null())
        .one(&db)
        .await?;

    match current {
        Some(app) => {
            let active = dtos::Application::ActiveModel {
                id: Set(app.id),
                name: Set(app.name),
                description: Set(app.description),
                deleted_at: Set(Some(chrono::Utc::now())),
                superseded_by: Set(app.superseded_by),
            };
            active.update(&db).await?;
            Ok(1)
        }
        None => Ok(0),
    }
}
