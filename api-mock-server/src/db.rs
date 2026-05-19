use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{ActiveModelTrait, ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use std::sync::OnceLock;

use crate::db_migration::Migrator;
use amos_common::entities::{Device, Group};

static DB: OnceLock<DatabaseConnection> = OnceLock::new();

fn db() -> &'static DatabaseConnection {
    DB.get().expect("Database not initialized")
}

pub async fn initalialize_db(database_url: String) -> Result<(), DbErr> {
    let mut opt = ConnectOptions::new(&database_url.to_owned());
    // SQL queries should be the last resort when debugging...
    opt.sqlx_logging_level(log::LevelFilter::Trace);

    let conn = Database::connect(opt).await?;

    conn.ping().await?;

    Migrator::up(&conn, None).await?;

    DB.set(conn)
        .map_err(|_| DbErr::Custom("DB already initialized".into()))?;

    Ok(())
}

pub async fn add_group(name: String) -> Result<i32, DbErr> {
    let group = Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
        // ..Default::default()
    };

    let new_group = group.insert(db()).await?;
    debug!("Inserted group: {:?}", new_group);

    return Ok(new_group.id);
}

pub async fn add_device(
    uuid: String,
    hostname: String,
    group_id: Option<i32>
) -> Result<i32, DbErr> {
    let device = Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        hostname: Set(hostname),
        group_id: Set(group_id),
    };

    let new_device = device.insert(db()).await?;
    debug!("Inserted device: {:?}", new_device);

    return Ok(new_device.id);
}
