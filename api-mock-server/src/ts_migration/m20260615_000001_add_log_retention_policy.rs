use sea_orm_migration::prelude::*;
use sea_orm_timescale::migration::{add_retention_policy, remove_retention_policy};
use sea_orm_timescale::types::{Interval, RetentionConfig};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let config = RetentionConfig {
            drop_after: Interval::Days(365),
        };

        add_retention_policy(manager.get_connection(), "device_logs", &config).await?;
        add_retention_policy(manager.get_connection(), "application_logs", &config).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        remove_retention_policy(manager.get_connection(), "device_logs").await?;
        remove_retention_policy(manager.get_connection(), "application_logs").await?;

        Ok(())
    }
}
