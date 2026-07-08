use sea_orm_migration::prelude::*;

const SOFT_DELETE_TABLES: &[&str] = &[
    "application_configs",
    "application_assignments",
    "os_versions",
    "os_assignments",
];

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != sea_orm::DbBackend::Postgres {
            return Ok(());
        }

        let db = manager.get_connection();

        for table in SOFT_DELETE_TABLES {
            db.execute_unprepared(&format!(
                "DO $$ BEGIN
                    ALTER TABLE {table} ADD COLUMN deleted_at TIMESTAMP WITH TIME ZONE DEFAULT NULL;
                EXCEPTION WHEN duplicate_column THEN
                    -- column already exists, skip
                END $$;"
            ))
            .await?;

            db.execute_unprepared(&format!(
                "DO $$ BEGIN
                    ALTER TABLE {table} ADD COLUMN superseded_by INTEGER DEFAULT NULL REFERENCES {table}(id);
                EXCEPTION WHEN duplicate_column THEN
                    -- column already exists, skip
                END $$;"
            ))
            .await?;

            db.execute_unprepared(&format!(
                "CREATE INDEX IF NOT EXISTS idx_{table}_active ON {table} (id) WHERE superseded_by IS NULL AND deleted_at IS NULL"
            ))
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DbBackend::Postgres {
            let db = manager.get_connection();

            for table in SOFT_DELETE_TABLES.iter().rev() {
                db.execute_unprepared(&format!("DROP INDEX IF EXISTS idx_{table}_active"))
                    .await?;
                db.execute_unprepared(&format!(
                    "ALTER TABLE {table} DROP COLUMN IF EXISTS superseded_by"
                ))
                .await?;
                db.execute_unprepared(&format!(
                    "ALTER TABLE {table} DROP COLUMN IF EXISTS deleted_at"
                ))
                .await?;
            }
        }

        Ok(())
    }
}
