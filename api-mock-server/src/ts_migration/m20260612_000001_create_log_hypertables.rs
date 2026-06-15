use sea_orm::Schema;
use sea_orm_migration::prelude::*;
use sea_orm_timescale::migration::create_hypertable;
use sea_orm_timescale::types::{HypertableConfig, Interval};

use crate::dtos;

use super::create_table;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());

        create_table(manager, &schema, dtos::DeviceLog::Entity).await?;
        create_hypertable(
            manager.get_connection(),
            &HypertableConfig {
                table_name: "device_logs".into(),
                time_column: "time".into(),
                chunk_interval: Some(Interval::Days(1)),
                if_not_exists: true,
            },
        )
        .await?;

        create_table(manager, &schema, dtos::ApplicationLog::Entity).await?;
        create_hypertable(
            manager.get_connection(),
            &HypertableConfig {
                table_name: "application_logs".into(),
                time_column: "time".into(),
                chunk_interval: Some(Interval::Days(1)),
                if_not_exists: true,
            },
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        _ = manager;
        todo!();
    }
}

// No retention policy is configured by default. To add one later, call
// `sea_orm_timescale::migration::add_retention_policy(db, "device_logs", &RetentionConfig {
//     drop_after: Interval::Days(30),
// })` (and similarly for `application_logs`) from a new migration.
