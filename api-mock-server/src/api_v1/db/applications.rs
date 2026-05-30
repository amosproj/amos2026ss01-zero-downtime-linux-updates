use amos_common::entities::Application;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Applications--

pub async fn list_applications(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Application::Model>, u64), DbErr> {
    let db = db!();
    let mut query = Application::Entity::find().order_by_asc(Application::Column::Id);
    if let Some(name) = name_filter {
        query = query.filter(Expr::col(Application::Column::Name).like(format!("%{}%", name)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn get_application(id: i32) -> Result<Option<Application::Model>, DbErr> {
    let db = db!();
    Application::Entity::find_by_id(id).one(&db).await
}

pub async fn add_application(
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let app = Application::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_app = app.insert(&db).await?;
    debug!("Inserted new application: {:?}", new_app);

    Ok(new_app)
}

pub async fn update_application(
    id: i32,
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let db = db!();
    let app = Application::ActiveModel {
        id: Set(id),
        name: Set(name),
        description: Set(description),
    };
    let updated_app = app.update(&db).await?;
    debug!("Updated application: {:?}", updated_app);
    Ok(updated_app)
}

pub async fn delete_application(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Application::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
