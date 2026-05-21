use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use tokio::sync::RwLock;

use crate::db_migration::Migrator;
use amos_common::entities::{
    Application, ApplicationAssignment, ApplicationConfig, Device, Group, Tenant,
};

static DB: RwLock<Option<DatabaseConnection>> = RwLock::const_new(None);

macro_rules! db {
    () => {
        DB.read().await.clone().unwrap()
    };
}

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

#[allow(dead_code)]
pub async fn add_tenant(name: String, description: Option<String>) -> Result<Tenant::Model, DbErr> {
    let tenant = Tenant::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_tenant = tenant.insert(&db).await?;
    debug!("Inserted new tenant: {:?}", new_tenant);

    Ok(new_tenant)
}

#[allow(dead_code)]
pub async fn add_group(name: String) -> Result<Group::Model, DbErr> {
    let group = Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
        // ..Default::default()
    };

    let db = db!();

    let new_group = group.insert(&db).await?;
    debug!("Inserted group: {:?}", new_group);

    Ok(new_group)
}

#[allow(dead_code)]
pub async fn add_device(
    uuid: String,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let device = Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_device = device.insert(&db).await?;
    debug!("Inserted device: {:?}", new_device);

    Ok(new_device)
}

#[allow(dead_code)]
pub async fn add_application(
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let app = Application::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_app = app.insert(&db).await?;
    debug!("Inserted new application: {:?}", new_app);

    Ok(new_app)
}

#[allow(dead_code)]
pub async fn add_application_config(
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let app_config = ApplicationConfig::ActiveModel {
        id: NotSet,
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
    };

    let db = db!();

    let new_app_config = app_config.insert(&db).await?;
    debug!("Inserted new application config: {:?}", new_app_config);

    Ok(new_app_config)
}

#[allow(dead_code)]
async fn add_application_assignment(
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let app_assignment = ApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    debug!(
        "Inserted new application config assignment: {:?}",
        new_app_assignment
    );

    Ok(new_app_assignment)
}

#[allow(dead_code)]
pub async fn add_application_assignment_to_device(
    app_config_id: i32,
    device_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, Some(device_id), None).await
}

#[allow(dead_code)]
pub async fn add_application_assignment_to_group(
    app_config_id: i32,
    group_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, Some(group_id), None).await
}

#[cfg(test)]
mod tests {
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
    async fn test_insert_app_assignement_without_group_or_device_fails() {
        test_initialize_empty_inmem_db().await;

        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();
        println!("Created app: {:?}", app);

        let app_config =
            super::add_application_config(app.id, "quay.io/bla".to_owned(), None, None)
                .await
                .unwrap();
        println!("Created app config: {:?}", app_config);

        let result = super::add_application_assignment(app_config.id, None, None).await;
        println!("Created application assignment: {:?}", result);

        assert!(result.is_err());
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
        let app: Result<super::Application::Model, _> = serde_json::from_str(app_json);

        assert!(app.is_ok());
    }
}
