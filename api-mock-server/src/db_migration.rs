pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{EntityTrait, Schema};

mod m20220101_000001_create_table;
mod m20260523_000001_add_reported_assignments;
mod m20260527_000001_add_device_pings;
mod m20260607_000001_add_users;
mod m20260614_000001_add_audit_log;
mod m20260622_000001_add_device_application_configs;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20260523_000001_add_reported_assignments::Migration),
            Box::new(m20260527_000001_add_device_pings::Migration),
            Box::new(m20260607_000001_add_users::Migration),
            Box::new(m20260614_000001_add_audit_log::Migration),
            Box::new(m20260622_000001_add_device_application_configs::Migration),
        ]
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
