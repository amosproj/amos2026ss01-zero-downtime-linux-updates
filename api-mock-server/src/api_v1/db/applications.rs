use amos_common::entities::Application;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, DbErr, EntityTrait};

use super::db;

// --Applications--

pub async fn list_applications() -> Result<Vec<Application::Model>, DbErr> {
    let db = db!();
    Application::Entity::find().all(&db).await
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
