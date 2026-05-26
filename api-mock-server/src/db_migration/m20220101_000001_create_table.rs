use sea_orm::Schema;
use sea_orm_migration::prelude::*;

use amos_common::entities;

use super::create_table;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_database_backend());

        // Don't switch table creation order randomly, it depends on foreign key presence
        // EDIT: ...in Postgres at least, it seems
        create_table(manager, &schema, entities::Tenant::Entity).await?;
        create_table(manager, &schema, entities::Group::Entity).await?;
        create_table(manager, &schema, entities::Device::Entity).await?;
        create_table(manager, &schema, entities::Application::Entity).await?;
        create_table(manager, &schema, entities::ApplicationConfig::Entity).await?;
        create_table(manager, &schema, entities::ApplicationAssignment::Entity).await?;
        create_table(manager, &schema, entities::OsVersion::Entity).await?;
        create_table(manager, &schema, entities::OsAssignment::Entity).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        _ = manager;
        todo!();
    }
}
