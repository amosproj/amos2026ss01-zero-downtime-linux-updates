pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{EntityTrait, Schema};

mod m20260612_000001_create_log_hypertables;

pub struct TsMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TsMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260612_000001_create_log_hypertables::Migration)]
    }
}

async fn create_table<E>(
    manager: &SchemaManager<'_>,
    schema: &Schema,
    entity: E,
) -> Result<(), DbErr>
where
    E: EntityTrait,
{
    let stmt = schema.create_table_from_entity(entity);
    manager.create_table(stmt).await
}
