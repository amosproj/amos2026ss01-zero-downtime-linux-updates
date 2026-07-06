mod db;
mod device;

type Router = axum::Router<db::DataStore>;

pub fn router(db_conn: sea_orm::DatabaseConnection) -> axum::Router {
    let data_store = db::DataStore::new(db_conn);

    axum::Router::new()
        .nest("/device", device::router())
        .with_state(data_store)
}
