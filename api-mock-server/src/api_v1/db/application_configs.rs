use amos_common::entities::ApplicationConfig;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter};

use super::db;

// --Application Configs--

pub async fn list_application_configs(
    application_id: Option<i32>,
) -> Result<Vec<ApplicationConfig::Model>, DbErr> {
    let db = db!();
    let mut query = ApplicationConfig::Entity::find();
    if let Some(id) = application_id {
        query = query.filter(ApplicationConfig::Column::ApplicationId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_application_config(id: i32) -> Result<Option<ApplicationConfig::Model>, DbErr> {
    let db = db!();
    ApplicationConfig::Entity::find_by_id(id).one(&db).await
}

pub async fn add_application_config(
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let app_config = ApplicationConfig::ActiveModel {
        id: NotSet,
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
    };

    let db = db!();

    let new_app_config = app_config.insert(&db).await?;
    debug!("Inserted new application config: {:?}", new_app_config);

    Ok(new_app_config)
}

pub async fn update_application_config(
    id: i32,
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let db = db!();
    let app_config = ApplicationConfig::ActiveModel {
        id: Set(id),
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
    };
    let updated_group = app_config.update(&db).await?;
    debug!("Updated application config: {:?}", updated_group);
    Ok(updated_group)
}

pub async fn delete_application_config(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ApplicationConfig::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}
