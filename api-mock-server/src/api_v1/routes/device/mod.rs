use axum::routing::{get, post, put};

use crate::auth::extractors::AuthDevice;

mod apps;
mod logs;
mod os;
mod ping;

/// Provides routes for the device-facing API, usually nested in ```/v2/device```.
/// Serves requests with a valid device JWT only.
pub fn router() -> axum::Router {
    axum::Router::new()
        .route("/apps", get(apps::get).put(apps::put))
        .route("/logs", post(logs::post))
        .route("/os", get(os::get).put(os::put))
        .route("/ping", put(ping::put))
        .route_layer(axum::middleware::from_extractor::<AuthDevice>())
}
