use amos_common::entities::{ApplicationLog, DeviceLog, LogEvent};

impl super::DataStore {
    pub async fn logs_publish_device(
        &self,
        device_id: i32,
        entries: Vec<DeviceLog::CreateEntry>,
    ) -> Result<(), sea_orm::DbErr> {
        let rows = crate::api_v1::ts_db::insert_device_log_entries(device_id, entries).await?;

        // Send log lines to real-time subscribers
        for row in rows {
            crate::api_v1::log_stream::publish(LogEvent::Device(row));
        }

        Ok(())
    }

    pub async fn logs_publish_application(
        &self,
        device_id: i32,
        application_id: i32,
        entries: Vec<ApplicationLog::CreateEntry>,
    ) -> Result<(), sea_orm::DbErr> {
        let rows = crate::api_v1::ts_db::insert_application_log_entries(
            device_id,
            application_id,
            entries,
        )
        .await?;

        // Send log lines to real-time subscribers
        for row in rows {
            crate::api_v1::log_stream::publish(LogEvent::Application(row));
        }

        Ok(())
    }
}
