use axum::routing::{get, post, put};

mod apps;
mod logs;
mod os;
mod ping;

pub fn router() -> super::Router {
    axum::Router::new()
        .route("/apps", get(apps::get).put(apps::put))
        .route("/logs", post(logs::post))
        .route("/os", get(os::get).put(os::put))
        .route("/ping", put(ping::put))
}
