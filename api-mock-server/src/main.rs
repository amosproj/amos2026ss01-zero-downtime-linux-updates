use clap::Parser;
mod api_v1;
mod auth;
mod config;
pub(crate) mod db_migration;
pub(crate) mod dtos;
pub(crate) mod ts_migration;
use axum::{Router, extract::Request, middleware as axum_middleware, routing::post};
mod audit_context;
use config::get_config;
use log::{debug, error, info};
use std::path::PathBuf;
use tokio::net::TcpListener;

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
    api_v1::db::initialialize_db(config.database_url, config.audit)
        .await
        .unwrap_or_else(|err| {
            error!("Failed to initialize database connection: {}", err);
            std::process::exit(1);
        });

    // Initialize TimescaleDB connection for time-series log data
    api_v1::ts_db::initialize_timescale_db(config.timescale_database_url)
        .await
        .unwrap_or_else(|err| {
            error!("Failed to initialize TimescaleDB connection: {}", err);
            std::process::exit(1);
        });

    let api_v1 = Router::new().merge(api_v1::routes::routes()).route_layer(
        axum::middleware::from_fn_with_state(config.jwt.clone(), auth::jwt_middleware),
    );

    // Device registration needs to be public as the device needs to register
    // its JWT pubkey before it can be used for verifying its signature
    let api_v1_public = Router::new().route(
        "/register-device",
        post(api_v1::routes::devices::register_device),
    );

    let app = Router::new()
        .nest("/v1", api_v1)
        .nest("/v1", api_v1_public)
        .layer(axum_middleware::from_fn(
            async |req: Request, next: axum_middleware::Next| {
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
