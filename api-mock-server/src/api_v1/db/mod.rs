pub mod application_assignments;
pub mod application_configs;
pub mod applications;
pub mod audit_log;
pub mod device_summaries;
pub mod devices;
pub mod groups;
pub mod os_assignments;
pub mod os_versions;
pub mod pings;
pub mod reported_application_assignments;
pub mod reported_os_assignments;
pub mod tenants;
pub mod users;

pub use application_assignments::*;
pub use application_configs::*;
pub use applications::*;
#[allow(unused_imports)]
pub use audit_log::*;
pub use device_summaries::*;
pub use devices::*;
pub use groups::*;
pub use os_assignments::*;
pub use os_versions::*;
pub use pings::*;
pub use reported_application_assignments::*;
pub use reported_os_assignments::*;
pub use tenants::*;
pub use users::*;

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use tokio::sync::RwLock;

use crate::db_migration::Migrator;

pub(crate) static DB: RwLock<Option<DatabaseConnection>> = RwLock::const_new(None);

macro_rules! db {
    () => {
        crate::api_v1::db::DB.read().await.clone().unwrap()
    };
}
pub(crate) use db;

pub async fn initialialize_db(database_url: String) -> Result<(), DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    // SQL queries should be the last resort when debugging...
    opt.sqlx_logging_level(log::LevelFilter::Trace);

    let conn = Database::connect(opt).await?;

    conn.ping().await?;

    Migrator::up(&conn, None).await?;

    DB.write().await.replace(conn);

    Ok(())
}

#[cfg(test)]
mod tests {
    use amos_common::entities::Application;
    use sea_orm::sea_query::prelude::serde_json;
    use serial_test::serial;

    #[cfg(test)]
    async fn test_initialize_empty_inmem_db() {
        super::initialialize_db("sqlite::memory:".into())
            .await
            .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_initialize_db_succeeds() {
        test_initialize_empty_inmem_db().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_device_with_existing_group_and_tenant_works() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant(
            "Kathis Käjsewelt".to_owned(),
            Some("Sitz: Nürnberg".to_owned()),
        )
        .await
        .unwrap();
        let group = super::add_group("Werk Erlangen #5".into()).await.unwrap();

        super::add_device(
            "c0ffee-xdxdxd-129874".to_owned(),
            "host-01.er5.weber.group".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_device_with_not_existing_group_fails() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("X".to_owned(), None).await.unwrap();

        let result = super::add_device(
            "c0ffee-xdxdxd-129874".to_owned(),
            "host-01.er5.weber.group".to_owned(),
            tenant.id,
            Some(0),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_app_assignement_for_device_works() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("X".to_owned(), None).await.unwrap();

        let device = super::add_device("".to_owned(), "".to_owned(), tenant.id, None)
            .await
            .unwrap();
        println!("Created device: {:?}", device);

        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();
        println!("Created app: {:?}", app);

        let app_config =
            super::add_application_config(app.id, "quay.io/bla".to_owned(), None, None)
                .await
                .unwrap();
        println!("Created app config: {:?}", app_config);

        let result = super::add_application_assignment_to_device(app_config.id, device.id).await;
        println!("Created application assignment: {:?}", result);

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn generated_application_model_json_matches_expected() {
        test_initialize_empty_inmem_db().await;

        let app = super::add_application("app-a".to_owned(), "cool app".to_owned())
            .await
            .unwrap();
        let app_json = serde_json::to_string(&app).unwrap();

        let expected = r#"{"id":1,"name":"app-a","description":"cool app"}"#;
        assert_eq!(app_json, expected);
    }

    #[tokio::test]
    #[serial]
    async fn given_application_model_json_unmarshalls() {
        test_initialize_empty_inmem_db().await;

        let app_json = r#"{"id":5,"name":"app-b","description":"mediocre app"}"#;
        let app: Result<Application::Model, _> = serde_json::from_str(app_json);

        assert!(app.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_list_devices_filtered_by_tenant() {
        test_initialize_empty_inmem_db().await;

        let t1 = super::add_tenant("T1".to_owned(), None).await.unwrap();
        let t2 = super::add_tenant("T2".to_owned(), None).await.unwrap();
        super::add_device("uuid-1".to_owned(), "host-1".to_owned(), t1.id, None)
            .await
            .unwrap();
        super::add_device("uuid-2".to_owned(), "host-2".to_owned(), t1.id, None)
            .await
            .unwrap();
        super::add_device("uuid-3".to_owned(), "host-3".to_owned(), t2.id, None)
            .await
            .unwrap();
        let (devices, _total) = super::list_devices(None, Some(t1.id), None, None, 0, 20)
            .await
            .unwrap();

        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_devices_filtered_by_group() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        super::add_device(
            "uuid-1".to_owned(),
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        super::add_device("uuid-2".to_owned(), "host-2".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let (devices, _total) = super::list_devices(Some(group.id), None, None, None, 0, 20)
            .await
            .unwrap();

        assert_eq!(devices.len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_application_assignment_reassign_to_different_device() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let d1 = super::add_device("uuid-d1".to_owned(), "host-d1".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let d2 = super::add_device("uuid-d2".to_owned(), "host-d2".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let app = super::add_application("app".to_owned(), "desc".to_owned())
            .await
            .unwrap();
        let config =
            super::add_application_config(app.id, "quay.io/app:1.0".to_owned(), None, None)
                .await
                .unwrap();
        let assignment = super::add_application_assignment_to_device(config.id, d1.id)
            .await
            .unwrap();
        let updated =
            super::update_application_assignment(assignment.id, config.id, Some(d2.id), None)
                .await
                .unwrap();

        assert_eq!(updated.device_id, Some(d2.id));
    }

    #[tokio::test]
    #[serial]
    async fn test_device_update_changes_hostname() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let device = super::add_device("uuid".to_owned(), "old-host".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let updated = super::update_device(
            device.id,
            device.uuid,
            "new-hostname".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();

        assert_eq!(updated.hostname, "new-hostname");
    }

    #[tokio::test]
    #[serial]
    async fn test_application_update_changes_description() {
        test_initialize_empty_inmem_db().await;

        let app = super::add_application("app".to_owned(), "old".to_owned())
            .await
            .unwrap();
        let updated = super::update_application(app.id, "app".to_owned(), "new".to_owned())
            .await
            .unwrap();

        assert_eq!(updated.description, "new");
    }

    #[tokio::test]
    #[serial]
    async fn test_device_summary_shape_and_contents() {
        test_initialize_empty_inmem_db().await;

        // -- Arrange --
        let tenant = super::add_tenant("Acme".to_owned(), Some("Sitz: Nürnberg".to_owned()))
            .await
            .unwrap();
        let device =
            super::add_device("uuid-abc".to_owned(), "host-01".to_owned(), tenant.id, None)
                .await
                .unwrap();

        // OS side
        let os_version = super::add_os_version(
            "deadbeef".to_owned(),
            "1.2.3".to_owned(),
            Some("stable release".to_owned()),
        )
        .await
        .unwrap();
        super::add_reported_os_assignment(os_version.id, device.id)
            .await
            .unwrap();

        // App side
        let app = super::add_application("my-app".to_owned(), "does things".to_owned())
            .await
            .unwrap();
        let config = super::add_application_config(
            app.id,
            "quay.io/my-app:1.0".to_owned(),
            Some(r#"{"port":8080}"#.to_owned()),
            Some("primary instance".to_owned()),
        )
        .await
        .unwrap();
        super::add_reported_application_assignment(config.id, device.id)
            .await
            .unwrap();

        // -- Act --
        let summary = super::get_device_summary(device.id)
            .await
            .unwrap()
            .expect("summary should exist for a known device id");

        // -- Assert: top-level keys --
        assert!(summary.get("device").is_some());
        assert!(summary.get("tenant").is_some());
        assert!(summary.get("os_versions").is_some());
        assert!(summary.get("applications").is_some());

        // -- Assert: device fields --
        assert_eq!(summary["device"]["uuid"], "uuid-abc");
        assert_eq!(summary["device"]["hostname"], "host-01");
        assert_eq!(summary["device"]["tenant_id"], tenant.id);
        assert_eq!(summary["device"]["group_id"], serde_json::Value::Null);

        // -- Assert: tenant fields --
        assert_eq!(summary["tenant"]["name"], "Acme");
        assert_eq!(summary["tenant"]["description"], "Sitz: Nürnberg");

        // -- Assert: os_versions entry --
        let os_versions = summary["os_versions"].as_array().unwrap();
        assert_eq!(os_versions.len(), 1);
        let os_entry = &os_versions[0];
        assert_eq!(os_entry["commit_hash"], "deadbeef");
        assert_eq!(os_entry["orchestrator_version"], "1.2.3");
        assert_eq!(os_entry["description"], "stable release");
        assert!(os_entry.get("reported_assignment_id").is_some());
        assert!(os_entry.get("updated_at").is_some());

        // -- Assert: applications entry --
        let applications = summary["applications"].as_array().unwrap();
        assert_eq!(applications.len(), 1);
        let app_entry = &applications[0];
        assert_eq!(app_entry["application_name"], "my-app");
        assert_eq!(app_entry["application_description"], "does things");
        assert_eq!(app_entry["image"], "quay.io/my-app:1.0");
        assert_eq!(app_entry["config"], r#"{"port":8080}"#);
        assert_eq!(app_entry["comment"], "primary instance");
        assert!(app_entry.get("reported_assignment_id").is_some());
        assert!(app_entry.get("updated_at").is_some());
    }
}
