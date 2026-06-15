use sea_orm::ActiveValue::Set;
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::prelude::chrono;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i64,

    pub table_name: String,

    pub record_id: String,

    pub operation: String,

    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub old_data: Option<Json>,

    #[sea_orm(column_type = "JsonBinary", nullable)]
    pub new_data: Option<Json>,

    pub changed_by: Option<String>,

    pub changed_at: DateTimeUtc,
}

#[async_trait::async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, _insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        self.changed_at = Set(chrono::Utc::now());
        Ok(self)
    }
}

impl Model {
    pub fn into_api(self) -> amos_common::entities::AuditLog::Model {
        amos_common::entities::AuditLog::Model {
            id: self.id,
            table_name: self.table_name,
            record_id: self.record_id,
            operation: self.operation,
            old_data: self.old_data,
            new_data: self.new_data,
            changed_by: self.changed_by,
            changed_at: self.changed_at,
        }
    }
}
