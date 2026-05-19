use sea_orm::{ConnectOptions, Database, DbErr};
use sea_orm_migration::MigratorTrait;

use crate::db_migration::Migrator;

pub async fn initalialize_db(database_url: String) -> Result<(), DbErr> {
    let mut opt = ConnectOptions::new(&database_url.to_owned());
    // SQL queries should be the last resort when debugging...
    opt.sqlx_logging_level(log::LevelFilter::Trace);

    let db = Database::connect(opt).await?;

    db.ping().await?;

    Migrator::up(&db, None).await?;

    Ok(())
}
