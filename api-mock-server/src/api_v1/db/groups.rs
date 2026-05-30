use amos_common::entities::Group;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, DbErr, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Groups--

pub async fn list_groups(
    name_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<Group::Model>, u64), DbErr> {
    let db = db!();
    let mut query = Group::Entity::find().order_by_asc(Group::Column::Id);
    if let Some(name) = name_filter {
        query = query.filter(Expr::col(Group::Column::Name).like(format!("%{}%", name)));
    }
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((data, total_items))
}

pub async fn get_group(id: i32) -> Result<Option<Group::Model>, DbErr> {
    let db = db!();
    Group::Entity::find_by_id(id).one(&db).await
}

pub async fn add_group(name: String) -> Result<Group::Model, DbErr> {
    let group = Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
        // ..Default::default()
    };

    let db = db!();

    let new_group = group.insert(&db).await?;
    debug!("Inserted group: {:?}", new_group);

    Ok(new_group)
}

pub async fn update_group(id: i32, name: String) -> Result<Group::Model, DbErr> {
    let db = db!();
    let group = Group::ActiveModel {
        id: Set(id),
        name: Set(name),
    };
    let updated_group = group.update(&db).await?;
    debug!("Updated group: {:?}", updated_group);
    Ok(updated_group)
}

pub async fn delete_group(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Group::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}
