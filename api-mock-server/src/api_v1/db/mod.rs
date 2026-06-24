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

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr};
use sea_orm_migration::MigratorTrait;
use tokio::sync::RwLock;

use crate::config::AuditConfig;
use crate::db_migration::Migrator;

pub(crate) static DB: RwLock<Option<DatabaseConnection>> = RwLock::const_new(None);

macro_rules! db {
    () => {
        crate::api_v1::db::DB.read().await.clone().unwrap()
    };
}
pub(super) use db;

pub async fn initialialize_db(
    database_url: String,
    audit_config: AuditConfig,
) -> Result<(), DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    // SQL queries should be the last resort when debugging...
    opt.sqlx_logging_level(log::LevelFilter::Trace);
    // Limit pool to 1 connection so that SET app.audit_user (used by audit
    // triggers) persists across all db.execute() calls within a single request.
    opt.max_connections(1);

    let conn = Database::connect(opt).await?;

    conn.ping().await?;

    Migrator::up(&conn, None).await?;

    reconcile_audit_triggers(&conn, &audit_config).await?;

    DB.write().await.replace(conn);

    Ok(())
}

const DEFAULT_AUDIT_TABLES: &[(&str, &str)] = &[
    ("tenants", "id"),
    ("groups", "id"),
    ("devices", "id"),
    ("applications", "id"),
    ("application_configs", "id"),
    ("application_assignments", "id"),
    ("os_versions", "id"),
    ("os_assignments", "id"),
    ("reported_application_assignments", "id"),
    ("reported_os_assignments", "id"),
    ("pings", "device_id"),
];

async fn reconcile_audit_triggers(
    conn: &DatabaseConnection,
    config: &AuditConfig,
) -> Result<(), DbErr> {
    if conn.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }

    let tracked: Vec<(String, String)> = match &config.tracked_tables {
        Some(tables) => {
            let mut result = Vec::new();
            for name in tables {
                let pk = DEFAULT_AUDIT_TABLES
                    .iter()
                    .find(|(t, _)| *t == name.as_str())
                    .map(|(_, pk)| *pk)
                    .unwrap_or("id");
                result.push((name.clone(), pk.to_string()));
            }
            result
        }
        None => DEFAULT_AUDIT_TABLES
            .iter()
            .map(|(t, pk)| (t.to_string(), pk.to_string()))
            .collect(),
    };

    for (table, _) in DEFAULT_AUDIT_TABLES {
        let trigger_name = format!("audit_{}", table);
        conn.execute_unprepared(&format!("DROP TRIGGER IF EXISTS {trigger_name} ON {table}"))
            .await?;
    }

    for (table, pk_col) in &tracked {
        let trigger_name = format!("audit_{}", table);
        let fn_name = if pk_col == "id" {
            "audit_log_trigger_fn"
        } else {
            "audit_log_trigger_pings_fn"
        };
        conn.execute_unprepared(&format!(
            "CREATE TRIGGER {trigger_name} AFTER INSERT OR UPDATE OR DELETE ON {table} FOR EACH ROW EXECUTE FUNCTION {fn_name}()"
        ))
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use amos_common::entities::{Application, ContainerConfigV1};
    use sea_orm::sea_query::prelude::serde_json;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    use serial_test::serial;

    use crate::config::AuditConfig;

    #[cfg(test)]
    async fn test_initialize_empty_inmem_db() {
        super::initialialize_db("sqlite::memory:".into(), AuditConfig::default())
            .await
            .unwrap();
    }

    #[cfg(test)]
    async fn test_initialize_postgres_db()
    -> testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres> {
        use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
        super::initialialize_db(url, AuditConfig::default())
            .await
            .unwrap();
        container // keep alive for the test's lifetime
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
            None,
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
            None,
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

        let device = super::add_device("".to_owned(), None, "".to_owned(), tenant.id, None)
            .await
            .unwrap();
        println!("Created device: {:?}", device);

        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();
        println!("Created app: {:?}", app);

        let app_config = super::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/bla".to_owned(),
            None,
        )
        .await
        .unwrap();
        println!("Created app config: {:?}", app_config);

        let result = super::add_application_assignment_to_device(app_config.id, device.id).await;
        println!("Created application assignment: {:?}", result);

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_application_config_crud_round_trip() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("X".to_owned(), None).await.unwrap();
        let device = super::add_device("uuid".to_owned(), None, "host".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();

        let default_config = ContainerConfigV1::default();

        let config = super::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/app".to_owned(),
            Some(default_config.clone()),
        )
        .await
        .unwrap();
        assert_eq!(config.version, 1);

        let fetched = super::get_application_config(config.id)
            .await
            .unwrap()
            .expect("config should exist");
        assert_eq!(fetched.config.unwrap(), default_config);

        let custom_config = ContainerConfigV1 {
            environment: Some(HashMap::from([("SOME_ENV".to_string(), "XXX".to_string())])),
        };

        let updated = super::update_application_config(
            config.id,
            Some(device.id),
            None,
            app.id,
            "quay.io/app".to_owned(),
            Some(custom_config.clone()),
        )
        .await
        .unwrap();
        assert_eq!(updated.version, 2);
        assert_eq!(updated.config.unwrap(), custom_config);

        let deleted = super::delete_application_config(updated.id).await.unwrap();
        assert_eq!(deleted, 1);
        assert!(
            super::get_application_config(updated.id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_list_application_configs_for_device_device_supersedes_group() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("X".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        let device = super::add_device(
            "uuid".to_owned(),
            None,
            "host".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();

        super::add_application_config(
            None,
            Some(group.id),
            app.id,
            "quay.io/app:group".to_owned(),
            None,
        )
        .await
        .unwrap();
        let device_config = super::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/app:device".to_owned(),
            None,
        )
        .await
        .unwrap();

        let resolved = super::list_application_configs_for_device(device.id)
            .await
            .unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, device_config.id);
        assert_eq!(resolved[0].image, "quay.io/app:device");
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
        super::add_device("uuid-1".to_owned(), None, "host-1".to_owned(), t1.id, None)
            .await
            .unwrap();
        super::add_device("uuid-2".to_owned(), None, "host-2".to_owned(), t1.id, None)
            .await
            .unwrap();
        super::add_device("uuid-3".to_owned(), None, "host-3".to_owned(), t2.id, None)
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
            None,
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        super::add_device(
            "uuid-2".to_owned(),
            None,
            "host-2".to_owned(),
            tenant.id,
            None,
        )
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
        let d1 = super::add_device(
            "uuid-d1".to_owned(),
            None,
            "host-d1".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();
        let d2 = super::add_device(
            "uuid-d2".to_owned(),
            None,
            "host-d2".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();
        let app = super::add_application("app".to_owned(), "desc".to_owned())
            .await
            .unwrap();
        let config = super::add_application_config(
            Some(d1.id),
            None,
            app.id,
            "quay.io/app:1.0".to_owned(),
            None,
        )
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
        assert_ne!(updated.id, assignment.id, "assignments use append-only");
    }

    #[tokio::test]
    #[serial]
    async fn test_application_config_update_uses_append_only() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T1".to_owned(), None).await.unwrap();

        let device = super::add_device("x".to_owned(), None, "host-03".to_owned(), tenant.id, None)
            .await
            .unwrap();

        let app = super::add_application("app".to_owned(), "desc".to_owned())
            .await
            .unwrap();
        let config = super::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/app:1.0".to_owned(),
            None,
        )
        .await
        .unwrap();
        let updated = super::update_application_config(
            config.id,
            Some(1),
            None,
            app.id,
            "quay.io/app:2.0".to_owned(),
            None,
        )
        .await
        .unwrap();

        assert_eq!(updated.image, "quay.io/app:2.0");
        assert_ne!(updated.id, config.id, "application configs use append-only");
    }

    #[tokio::test]
    #[serial]
    async fn test_os_version_update_uses_append_only() {
        test_initialize_empty_inmem_db().await;

        let version = super::add_os_version("deadbeef".to_owned(), "1.0.0".to_owned(), None)
            .await
            .unwrap();
        let updated =
            super::update_os_version(version.id, "cafebabe".to_owned(), "2.0.0".to_owned(), None)
                .await
                .unwrap();

        assert_eq!(updated.commit_hash, "cafebabe");
        assert_eq!(updated.orchestrator_version, "2.0.0");
        assert_ne!(updated.id, version.id, "os versions use append-only");
    }

    #[tokio::test]
    #[serial]
    async fn test_device_update_changes_serial_number() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let device = super::add_device(
            "uuid".to_owned(),
            None,
            "QW12ERTY".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();
        let updated = super::update_device(
            device.id,
            device.uuid,
            None,
            "UI89OPLK".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();

        assert_eq!(updated.serial_number, "UI89OPLK");
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
        let device = super::add_device(
            "uuid-abc".to_owned(),
            None,
            "host-01".to_owned(),
            tenant.id,
            None,
        )
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

        let custom_config = ContainerConfigV1 {
            environment: Some(HashMap::from([("PORT".to_string(), "8080".to_string())])),
        };

        let config = super::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/my-app:1.0".to_owned(),
            Some(custom_config),
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
        assert_eq!(summary["device"]["serial_number"], "host-01");
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
        assert_eq!(app_entry["config"], r#"{"environment":{"PORT":"8080"}}"#);
        assert_eq!(app_entry["version"], 1);
        assert!(app_entry.get("reported_assignment_id").is_some());
        assert!(app_entry.get("updated_at").is_some());
    }

    // Application_assignments for device tests
    #[tokio::test]
    #[serial]
    async fn test_app_assignments_for_device_device_wins_over_group_for_same_application() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        let device = super::add_device(
            "uuid-1".to_owned(),
            None,
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        let app = super::add_application("App".to_owned(), "desc".to_owned())
            .await
            .unwrap();

        // Group-level config and assignment for the same application
        let group_config = super::add_application_config(
            None,
            Some(group.id),
            app.id,
            "quay.io/app:group".to_owned(),
            None,
        )
        .await
        .unwrap();
        super::add_application_assignment_to_group(group_config.id, group.id)
            .await
            .unwrap();

        // Device-level config and assignment for the same application
        let device_config = super::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/app:device".to_owned(),
            None,
        )
        .await
        .unwrap();
        super::add_application_assignment_to_device(device_config.id, device.id)
            .await
            .unwrap();

        let (assignments, total) =
            super::list_application_assignments_for_device(device.id, Some(group.id), None, 0, 20)
                .await
                .unwrap();

        // Only one assignment must come back — the device-level one
        assert_eq!(total, 1);
        assert_eq!(assignments[0].application_config_id, device_config.id);
        assert_eq!(assignments[0].device_id, Some(device.id));
    }

    #[tokio::test]
    #[serial]
    async fn test_app_assignments_for_device_includes_both_when_different_applications() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        let device = super::add_device(
            "uuid-1".to_owned(),
            None,
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        let app1 = super::add_application("App1".to_owned(), "first".to_owned())
            .await
            .unwrap();
        let app2 = super::add_application("App2".to_owned(), "second".to_owned())
            .await
            .unwrap();

        // Group assignment for app1
        let group_config = super::add_application_config(
            None,
            Some(group.id),
            app1.id,
            "quay.io/app1:group".to_owned(),
            None,
        )
        .await
        .unwrap();
        super::add_application_assignment_to_group(group_config.id, group.id)
            .await
            .unwrap();

        // Device assignment for app2 (different application — no conflict)
        let device_config = super::add_application_config(
            Some(device.id),
            None,
            app2.id,
            "quay.io/app2:device".to_owned(),
            None,
        )
        .await
        .unwrap();
        super::add_application_assignment_to_device(device_config.id, device.id)
            .await
            .unwrap();

        let (assignments, total) =
            super::list_application_assignments_for_device(device.id, Some(group.id), None, 0, 20)
                .await
                .unwrap();

        // Both must be returned since they cover different applications
        assert_eq!(total, 2);
        let config_ids: Vec<i32> = assignments
            .iter()
            .map(|a| a.application_config_id)
            .collect();
        assert!(config_ids.contains(&device_config.id));
        assert!(config_ids.contains(&group_config.id));
    }

    // OsApplication Assignment for device tests
    #[tokio::test]
    #[serial]
    async fn test_os_assignments_for_device_returns_group_assignment_when_no_device_assignment() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        let device = super::add_device(
            "uuid-1".to_owned(),
            None,
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        let os = super::add_os_version("deadbeef".to_owned(), "1.0.0".to_owned(), None)
            .await
            .unwrap();
        super::add_os_assignment(os.id, None, Some(group.id))
            .await
            .unwrap();

        let (assignments, total) =
            super::list_os_assignments_for_device(device.id, Some(group.id), None, 0, 20)
                .await
                .unwrap();

        assert_eq!(total, 1);
        assert_eq!(assignments[0].os_version_id, os.id);
        assert_eq!(assignments[0].group_id, Some(group.id));
    }

    #[tokio::test]
    #[serial]
    async fn test_os_assignments_for_device_device_wins_over_group() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        let device = super::add_device(
            "uuid-1".to_owned(),
            None,
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        let group_os = super::add_os_version("aabbccdd".to_owned(), "1.0.0".to_owned(), None)
            .await
            .unwrap();
        let device_os = super::add_os_version("deadbeef".to_owned(), "2.0.0".to_owned(), None)
            .await
            .unwrap();

        super::add_os_assignment(group_os.id, None, Some(group.id))
            .await
            .unwrap();
        super::add_os_assignment(device_os.id, Some(device.id), None)
            .await
            .unwrap();

        let (assignments, total) =
            super::list_os_assignments_for_device(device.id, Some(group.id), None, 0, 20)
                .await
                .unwrap();

        // Only the device-level assignment must be returned
        assert_eq!(total, 1);
        assert_eq!(assignments[0].os_version_id, device_os.id);
        assert_eq!(assignments[0].device_id, Some(device.id));
    }

    // Integration tests for audit log functionality.
    // These require PostgreSQL (triggers do not fire on SQLite).
    // Run with: cargo test -- --ignored (against a PostgreSQL instance)

    #[tokio::test]
    #[serial]
    async fn audit_log_captures_insert() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();

        let entries =
            super::audit_log::get_audit_logs_for_record("tenants", &tenant.id.to_string())
                .await
                .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].operation, "INSERT");
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_captures_update() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        super::update_tenant(tenant.id, "T2".to_owned(), None)
            .await
            .unwrap();

        let entries =
            super::audit_log::get_audit_logs_for_record("tenants", &tenant.id.to_string())
                .await
                .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].operation, "UPDATE");
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_captures_delete() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        super::delete_tenant(tenant.id).await.unwrap();

        let entries =
            super::audit_log::get_audit_logs_for_record("tenants", &tenant.id.to_string())
                .await
                .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].operation, "DELETE");
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_changed_by_null_for_unauthenticated() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();

        let entries =
            super::audit_log::get_audit_logs_for_record("tenants", &tenant.id.to_string())
                .await
                .unwrap();
        assert_eq!(entries.len(), 1);

        let db = super::DB.read().await.clone().unwrap();
        let system_user = crate::dtos::User::Entity::find()
            .filter(crate::dtos::User::Column::Subject.eq("system"))
            .one(&db)
            .await
            .unwrap()
            .expect("system user should exist");
        assert_eq!(entries[0].changed_by, system_user.id);
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_full_history_for_record() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let device = super::add_device("uuid".to_owned(), None, "host".to_owned(), tenant.id, None)
            .await
            .unwrap();
        super::update_device(
            device.id,
            device.uuid.clone(),
            None,
            "host2".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();
        super::update_device(
            device.id,
            device.uuid,
            None,
            "host3".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();

        let entries =
            super::audit_log::get_audit_logs_for_record("devices", &device.id.to_string())
                .await
                .unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].operation, "INSERT");
        assert_eq!(entries[1].operation, "UPDATE");
        assert_eq!(entries[2].operation, "UPDATE");
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_device_history() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let device = super::add_device("uuid".to_owned(), None, "host".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let app = super::add_application("app".to_owned(), "desc".to_owned())
            .await
            .unwrap();
        let config =
            super::add_application_config(Some(device.id), None, app.id, "img".to_owned(), None)
                .await
                .unwrap();
        super::add_application_assignment_to_device(config.id, device.id)
            .await
            .unwrap();

        let (entries, _) = super::audit_log::get_audit_logs_for_device(device.id, 0, 20)
            .await
            .unwrap();
        assert!(entries.len() >= 2);
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_list_with_filters() {
        let _container = test_initialize_postgres_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        super::add_device("uuid".to_owned(), None, "host".to_owned(), tenant.id, None)
            .await
            .unwrap();

        let (entries, _) =
            super::audit_log::list_audit_logs(Some("tenants".to_string()), None, None, None, 0, 20)
                .await
                .unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].table_name, "tenants");
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_pagination() {
        let _container = test_initialize_postgres_db().await;

        for i in 0..25 {
            super::add_tenant(format!("T{i}"), None).await.unwrap();
        }

        let (_, total) = super::audit_log::list_audit_logs(None, None, None, None, 0, 10)
            .await
            .unwrap();
        assert!(total >= 25);
    }

    #[tokio::test]
    #[serial]
    async fn audit_log_configurable_tables() {
        use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

        super::initialialize_db(
            url,
            AuditConfig {
                tracked_tables: Some(vec!["devices".to_string()]),
            },
        )
        .await
        .unwrap();

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        super::add_device("uuid".to_owned(), None, "host".to_owned(), tenant.id, None)
            .await
            .unwrap();

        let (tenant_entries, _) =
            super::audit_log::list_audit_logs(Some("tenants".to_string()), None, None, None, 0, 20)
                .await
                .unwrap();
        assert_eq!(tenant_entries.len(), 0);

        let (device_entries, _) =
            super::audit_log::list_audit_logs(Some("devices".to_string()), None, None, None, 0, 20)
                .await
                .unwrap();
        assert_eq!(device_entries.len(), 1);
    }
}
