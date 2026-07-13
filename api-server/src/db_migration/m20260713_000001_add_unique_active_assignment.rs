use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != sea_orm::DbBackend::Postgres {
            return Ok(());
        }
        let db = manager.get_connection();

        // os_assignments: at most one active assignment per device, and per group
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_os_assignments_active_device
             ON os_assignments (device_id)
             WHERE superseded_by IS NULL AND deleted_at IS NULL AND device_id IS NOT NULL",
        )
        .await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_os_assignments_active_group
             ON os_assignments (group_id)
             WHERE superseded_by IS NULL AND deleted_at IS NULL AND group_id IS NOT NULL",
        )
        .await?;

        // application_configs: at most one active config per (application, device/group)
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_app_configs_active_device
             ON application_configs (application_id, device_id)
             WHERE superseded_by IS NULL AND deleted_at IS NULL AND device_id IS NOT NULL",
        )
        .await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_app_configs_active_group
             ON application_configs (application_id, group_id)
             WHERE superseded_by IS NULL AND deleted_at IS NULL AND group_id IS NOT NULL",
        )
        .await?;

        // application_assignments: at most one active assignment per (config, device/group)
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_app_assignments_active_device
             ON application_assignments (application_config_id, device_id)
             WHERE superseded_by IS NULL AND deleted_at IS NULL AND device_id IS NOT NULL",
        )
        .await?;
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS uq_app_assignments_active_group
             ON application_assignments (application_config_id, group_id)
             WHERE superseded_by IS NULL AND deleted_at IS NULL AND group_id IS NOT NULL",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == sea_orm::DbBackend::Postgres {
            let db = manager.get_connection();
            for idx in [
                "uq_os_assignments_active_device",
                "uq_os_assignments_active_group",
                "uq_app_configs_active_device",
                "uq_app_configs_active_group",
                "uq_app_assignments_active_device",
                "uq_app_assignments_active_group",
            ] {
                db.execute_unprepared(&format!("DROP INDEX IF EXISTS {idx}"))
                    .await?;
            }
        }
        Ok(())
    }
}
