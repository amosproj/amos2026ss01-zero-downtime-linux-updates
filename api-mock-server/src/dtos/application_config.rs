use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::Application;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "application_configs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub application_id: i32,
    #[sea_orm(belongs_to, from = "application_id", to = "id")]
    pub application: HasOne<Application::Entity>,

    pub image: String,

    pub config: Option<String>,

    pub comment: Option<String>,

    pub deleted_at: Option<DateTimeUtc>,

    pub superseded_by: Option<i32>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::ApplicationConfig::Model {
        amos_common::entities::ApplicationConfig::Model {
            id: self.id,
            application_id: self.application_id,
            image: self.image,
            config: self.config,
            comment: self.comment,
            deleted_at: self.deleted_at,
            superseded_by: self.superseded_by,
        }
    }
}
