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

        // Don't switch table creation order randomly, it depends on foreign key presence
        // EDIT: ...in Postgres at least, it seems
        create_table(manager, &schema, dtos::Tenant::Entity).await?;
        create_table(manager, &schema, dtos::Group::Entity).await?;
        create_table(manager, &schema, dtos::Device::Entity).await?;
        create_table(manager, &schema, dtos::Application::Entity).await?;
        create_table(manager, &schema, dtos::ApplicationConfig::Entity).await?;
        create_table(manager, &schema, dtos::ApplicationAssignment::Entity).await?;
        create_table(manager, &schema, dtos::OsVersion::Entity).await?;
        create_table(manager, &schema, dtos::OsAssignment::Entity).await?;

        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX idx_application_configs_device_app \
                 ON application_configs (device_id, application_id)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        _ = manager;
        todo!();
    }
}
