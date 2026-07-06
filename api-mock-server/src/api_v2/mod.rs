use axum::routing::post;

mod db;
mod device;
mod register;

type Router = axum::Router<db::DataStore>;

pub fn router(
    db_conn: sea_orm::DatabaseConnection,
    jwt_config: crate::config::JwtConfig,
) -> axum::Router {
    let data_store = db::DataStore::new(db_conn);

    axum::Router::new()
        .nest("/device", device::router())
        .route_layer(axum::middleware::from_fn_with_state(
            jwt_config,
            crate::auth::jwt_middleware,
        ))
        // Register endpoint has to be unprotected
        .route("/register", post(register::post))
        .with_state(data_store)
}
