use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use tokio::sync::RwLock;

use crate::db_migration::Migrator;
use amos_common::entities::{Device, Group};

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
pub async fn add_group(name: String) -> Result<i32, DbErr> {
    let group = Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
        // ..Default::default()
    };

    let db = db!();

    let new_group = group.insert(&db).await?;
    debug!("Inserted group: {:?}", new_group);

    Ok(new_group.id)
}

#[allow(dead_code)]
pub async fn add_device(
    uuid: String,
    hostname: String,
    group_id: Option<i32>,
) -> Result<i32, DbErr> {
    let device = Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        hostname: Set(hostname),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_device = device.insert(&db).await?;
    debug!("Inserted device: {:?}", new_device);

    Ok(new_device.id)
}

#[cfg(test)]
mod tests {
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
    async fn test_insert_device_with_existing_group() {
        test_initialize_empty_inmem_db().await;

        let gid = super::add_group("Wurschtwerk Erlangen #5".into())
            .await
            .unwrap();
        super::add_device(
            "c0ffee-xdxdxd-129874".to_owned(),
            "host-01.er5.weber.group".to_owned(),
            Some(gid),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_device_with_not_existing_group_fails() {
        test_initialize_empty_inmem_db().await;

        let result = super::add_device(
            "c0ffee-xdxdxd-129874".to_owned(),
            "host-01.er5.weber.group".to_owned(),
            Some(0),
        )
        .await;

        assert!(result.is_err());
    }
}
