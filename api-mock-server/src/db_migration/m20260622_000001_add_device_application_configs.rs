use sea_orm::Schema;
use sea_orm_migration::prelude::*;

use crate::dtos;

use super::create_table;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());

        create_table(manager, &schema, dtos::DeviceApplicationConfig::Entity).await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX idx_device_application_configs_device_app \
                 ON device_application_configs (device_id, application_id)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        _ = manager;
        todo!();
    }
}
