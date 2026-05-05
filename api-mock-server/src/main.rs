use amos_common::{api, util};
use axum::{Json, Router, routing::get};
use tower_http::services::ServeDir;

static CATALOG_RES: api::CatalogResponse = api::CatalogResponse {
    os: api::CatalogResponseEntry {
        version: "1.2.3",
        url: "/v1/download/os1.2.3",
        signature: util::Base64::from_slice(&[0u8; 16]),
    },
    app: api::CatalogResponseEntry {
        version: "4.5.6",
        url: "/v1/download/app4.5.6",
        signature: util::Base64::from_slice(&[0u8; 16]),
    },
};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let api_v1 = Router::new()
        .route("/catalog", get(|| async { Json(&CATALOG_RES) }))
        .nest_service("/download", ServeDir::new("assets"));

    let app = Router::new().nest("/v1", api_v1);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:80").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
