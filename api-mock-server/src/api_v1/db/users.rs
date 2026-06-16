use crate::auth_user::Claims;
use crate::dtos;
use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{DbErr, EntityTrait};

use super::db;

// --Users--

pub async fn upsert_user(claims: Claims) -> Result<(), DbErr> {
    let db = db!();

    let user = dtos::User::ActiveModel {
        id: NotSet,
        subject: Set(claims.subject),
        name: Set(claims.name),
    };

    let upserted_user = dtos::User::Entity::insert(user)
        .on_conflict(
            OnConflict::column(dtos::User::Column::Subject)
                .update_column(dtos::User::Column::Name)
                .to_owned(),
        )
        .exec_with_returning(&db)
        .await?;

    debug!("Upserted user: {:?}", upserted_user);

    Ok(())
}
