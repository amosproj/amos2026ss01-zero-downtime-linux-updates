use amos_common::{api, util};
use axum::{Json, Router, extract::Request, middleware, routing::get};
use tower_http::services::ServeDir;

static CATALOG: [api::CatalogResponseEntry; 2] = [
    api::CatalogResponseEntry {
        name: "os",
        version: "1.2.3",
        url: "/v1/download/os1.2.3",
        signature: util::Base64::from_slice(&[0u8; 16]),
    },
    api::CatalogResponseEntry {
        name: "app",
        version: "4.5.6",
        url: "/v1/download/app4.5.6",
        signature: util::Base64::from_slice(&[0u8; 16]),
    },
];

static CATALOG_RES: api::CatalogResponse = api::CatalogResponse::from_slice(&CATALOG);

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let api_v1 = Router::new()
        .route("/catalog", get(|| async { Json(&CATALOG_RES) }))
        .nest_service("/download", ServeDir::new("assets"));

    let app = Router::new().nest("/v1", api_v1).layer(middleware::from_fn(
        async |req: Request, next: middleware::Next| {
            let uri = req.uri().to_string();
            let res = next.run(req).await;
            println!("{} -> {}", uri, res.status());
            res
        },
    ));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:80").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
