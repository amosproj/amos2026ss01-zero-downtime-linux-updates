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

        create_table(
            manager,
            &schema,
            entities::ReportedApplicationAssignment::Entity,
        )
        .await?;
        create_table(manager, &schema, entities::ReportedOsAssignment::Entity).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        _ = manager;
        todo!();
    }
}
