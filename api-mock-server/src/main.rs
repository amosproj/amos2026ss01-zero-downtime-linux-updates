use clap::Parser;
mod api_v1;
mod config;
pub(crate) mod db_migration;
use amos_common::inventory_model::{
    ApplicationInfo, BootcDeploymentInfo, BootcImageInfo, BootcStatus, SystemInfo,
    SystemRequirements,
};
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

// Mocked cloud-side target state. The shape mirrors the orchestrator's Device
// Inventory MVP JSON (see orchestrator/src/inventory.rs) so the orchestrator's
// update-check can compare local vs. target field-by-field.
fn mock_system_requirements() -> SystemRequirements {
    SystemRequirements {
        system: SystemInfo {
            hostname: "any".into(),
            os_name: "Fedora Linux".into(),
            os_version: "41".into(),
            kernel_version: "6.11.0".into(),
        },
        bootc_status: BootcStatus {
            booted: BootcDeploymentInfo {
                checksum: "0000000000000000000000000000000000000000000000000000000000000000".into(),
                image: Some(BootcImageInfo {
                    image_ref: "ghcr.io/amosproj/amos2026ss01-zero-downtime-linux-updates-system"
                        .into(),
                    transport: "registry".into(),
                    image_digest: None,
                    version: Some("1.2.3".into()),
                }),
            },
            staged: None,
            rollback: None,
            rollback_queued: false,
        },
        applications: vec![ApplicationInfo {
            app_name: "data_collector".into(),
            app_version: "v1.0.2".into(),
        }],
    }
}

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
        .route(
            "/system-requirements",
            get(|| async { Json(mock_system_requirements()) }),
        )
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
