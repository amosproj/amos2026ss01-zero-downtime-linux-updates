use sea_orm_migration::prelude::*;

use crate::dtos;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("idx_reported_app_assignment_device_config_unique")
                    .table(dtos::ReportedApplicationAssignment::Entity)
                    .col(dtos::ReportedApplicationAssignment::Column::DeviceId)
                    .col(dtos::ReportedApplicationAssignment::Column::ApplicationConfigId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_reported_os_assignment_device_osversion_unique")
                    .table(dtos::ReportedOsAssignment::Entity)
                    .col(dtos::ReportedOsAssignment::Column::DeviceId)
                    .col(dtos::ReportedOsAssignment::Column::OsVersionId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        _ = manager;
        todo!();
    }
}
