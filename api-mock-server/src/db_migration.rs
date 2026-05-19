pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{EntityTrait, Schema};

mod m20220101_000001_create_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20220101_000001_create_table::Migration)]
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
