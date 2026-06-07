use clap::Parser;
mod api_v1;
mod config;
pub(crate) mod db_migration;
pub(crate) mod dtos;
use amos_common::{api, util};
use axum::{Json, Router, extract::Request, middleware, routing::get};
use config::get_config;
use log::{debug, error, info};
use std::path::PathBuf;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

static CATALOG: [api::CatalogResponseEntry; 2] = [
    api::CatalogResponseEntry {
        name: "os",
        version: "1.2.3",
        url: "ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system",
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

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "Server")]
#[command(version = VERSION)]
#[command(about = "Provides an API for Orchestrators to query the desired OS and application state", long_about = None)]
struct Cli {
    /// Sets a custom config file path
    #[arg(short, long, value_name = "FILE")]
    pub config: Option<PathBuf>,

    /// Turn debugging information on
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub debug: u8,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let cli = Cli::parse();

    // Adjust log level according to verbosity specified via CLI
    let mut log_level = log::LevelFilter::Warn;
    for _ in 0..cli.debug {
        log_level = log_level.increment_severity();
    }

    env_logger::builder().filter_level(log_level).init();

    info!("Started not-so-mock-server...");

    let config = get_config(cli.config).unwrap_or_else(|err| {
        error!("Failed to load config: {}", err);
        std::process::exit(1);
    });

    // Initialize database
    api_v1::db::initialialize_db(config.database_url)
        .await
        .unwrap_or_else(|err| {
            error!("Failed to initialize database connection: {}", err);
            std::process::exit(1);
        });

    let api_v1 = Router::new()
        .route("/catalog", get(|| async { Json(&CATALOG_RES) }))
        .nest_service("/download", ServeDir::new("assets"))
        .merge(api_v1::routes::routes());

    let app = Router::new().nest("/v1", api_v1).layer(middleware::from_fn(
        async |req: Request, next: middleware::Next| {
            let uri = req.uri().to_string();
            let res = next.run(req).await;
            debug!("{} -> {}", uri, res.status());
            res
        },
    ));

    let bind_address = format!("0.0.0.0:{}", config.http_port);
    let listener = TcpListener::bind(&bind_address)
        .await
        .unwrap_or_else(|err| {
            error!("Could not start server: {}", err);
            std::process::exit(1);
        });
    info!("Serving API on {}", bind_address);

    axum::serve(listener, app).await.unwrap();
}

// An attempt to recreate test_server.sh
#[cfg(test)]
mod integration_tests {
    use super::*;
    use axum::body::Body;
    use serde_json::json;
    use serial_test::serial;
    use tower::ServiceExt;

    async fn setup_test_app() -> Router {
        crate::api_v1::db::initialialize_db("sqlite::memory:".to_string())
            .await
            .expect("Failed to initialize test in-memory database instance");

        let api_v1 = Router::new()
            .route("/catalog", get(|| async { Json(&CATALOG_RES) }))
            .nest_service("/download", ServeDir::new("assets"))
            .merge(crate::api_v1::routes::routes());

        Router::new().nest("/v1", api_v1).layer(middleware::from_fn(
            async |req: axum::extract::Request, next: middleware::Next| {
                let uri = req.uri().to_string();
                let res = next.run(req).await;
                res
            },
        ))
    }

    async fn assert_api(
        app: &Router,
        method: &str,
        uri: &str,
        body: Option<serde_json::Value>,
        expected_status: u16,
    ) {
        let req_body = match body {
            Some(json_val) => Body::from(json_val.to_string()),
            None => Body::empty(),
        };

        let request = axum::http::Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(req_body)
            .unwrap();

        let response = app.clone().oneshot(request).await.unwrap();

        assert_eq!(
            response.status().as_u16(),
            expected_status,
            "FAIL [{} {}]: expected status {}, got {}",
            method,
            uri,
            expected_status,
            response.status().as_u16()
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_server_compatibility_suite() {
        let app = setup_test_app().await;

        // --- 1. Tenants ---
        assert_api(
            &app,
            "POST",
            "/v1/tenants",
            Some(json!({
                "name": "Weber-Lager",
                "description": "Meta-Ort für initialisierte unverschickte Geräte"
            })),
            201,
        )
        .await;

        assert_api(
            &app,
            "POST",
            "/v1/tenants",
            Some(json!({
                "name": "Kaufland-Fabrik-Erlangen",
                "description": "Stammkunde in Deutschland"
            })),
            201,
        )
        .await;

        assert_api(
            &app,
            "POST",
            "/v1/tenants",
            Some(json!({
                "name": "7-Eleven-Fabrik-Tokyo",
                "description": "Zentrale Stelle in Chiyoda für Tokyo"
            })),
            201,
        )
        .await;

        assert_api(
            &app,
            "POST",
            "/v1/tenants",
            Some(json!({
                "name": "Foodland-Fabrik-Bangkok",
                "description": "Hauptlagerort in Bangkok"
            })),
            201,
        )
        .await;

        // --- 2. Devices ---
        assert_api(
            &app,
            "POST",
            "/v1/devices",
            Some(json!({
                "uuid": "8b722f94-6852-42cf-9722-98446499a457",
                "hostname": "x38974",
                "tenant_id": 1
            })),
            201,
        )
        .await;

        // --- 3. OS Versions ---
        assert_api(
            &app,
            "POST",
            "/v1/os-versions",
            Some(json!({
                "commit_hash": "092599a804d5169ae2a0a306bcb4b213b7646d28",
                "orchestrator_version": "0.1.0",
                "description": "First stable release, tested intensively"
            })),
            201,
        )
        .await;

        // --- 4. OS Assignments ---
        assert_api(
            &app,
            "POST",
            "/v1/os-assignments",
            Some(json!({
                "os_version_id": 1,
                "device_id": 1
            })),
            201,
        )
        .await;

        // --- 5. Reported OS Assignments (Standard Body) ---
        assert_api(
            &app,
            "POST",
            "/v1/reported-os-assignments",
            Some(json!({
                "os_version_id": 1,
                "device_id": 1
            })),
            201,
        )
        .await;

        // --- 6. Reported OS Assignments (URL Query String Target) ---
        assert_api(
            &app,
            "POST",
            "/v1/reported-os-assignments?device_uuid=8b722f94-6852-42cf-9722-98446499a457",
            Some(json!({
                "os_version_id": 1
            })),
            201,
        )
        .await;

        // --- 7. Edge Case: Missing Device Response Verification ---
        assert_api(
            &app,
            "POST",
            "/v1/reported-os-assignments?device_uuid=00000000-0000-0000-0000-000000000000",
            Some(json!({
                "os_version_id": 1
            })),
            404,
        )
        .await;
    }
}
