use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::LogLevel;

/// `device_id` references a row in the *main* Postgres database, but this
/// entity lives in the TimescaleDB database. A real FK constraint across
/// databases is impossible, so this is a plain `i32` column with no
/// `belongs_to`/`HasOne` relation.
#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "device_logs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub time: DateTimeUtc,
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub device_id: i32,
    pub level: LogLevel,
    pub message: String,
    pub source: Option<String>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::DeviceLog::Model {
        amos_common::entities::DeviceLog::Model {
            id: self.id,
            time: self.time,
            device_id: self.device_id,
            level: self.level.into(),
            message: self.message,
            source: self.source,
        }
    }
}
