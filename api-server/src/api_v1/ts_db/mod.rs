pub mod application_logs;
pub mod device_logs;

pub use application_logs::*;
pub use device_logs::*;

use amos_common::entities::LogLevel;
use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use tokio::sync::RwLock;

use crate::dtos;
use crate::ts_migration::TsMigrator;

/// Returns the dtos `LogLevel` variants whose severity is `>= min`, for use
/// with a sea-orm `is_in` filter. Severity order does not match the
/// alphabetical order of the stored string values, so a plain `>=` filter on
/// the database column would be incorrect.
pub(super) fn levels_at_least(min: LogLevel) -> Vec<dtos::LogLevel> {
    [
        LogLevel::Trace,
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
        LogLevel::Fatal,
    ]
    .into_iter()
    .filter(|level| *level >= min)
    .map(dtos::LogLevel::from)
    .collect()
}

pub(super) static TS_DB: RwLock<Option<DatabaseConnection>> = RwLock::const_new(None);

macro_rules! ts_db {
    () => {
        crate::api_v1::ts_db::TS_DB.read().await.clone().unwrap()
    };
}
pub(super) use ts_db;

pub async fn initialize_timescale_db(database_url: String) -> Result<(), DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    // SQL queries should be the last resort when debugging...
    opt.sqlx_logging_level(log::LevelFilter::Trace);

    let conn = Database::connect(opt).await?;

    conn.ping().await?;

    TsMigrator::up(&conn, None).await?;

    TS_DB.write().await.replace(conn);

    Ok(())
}
