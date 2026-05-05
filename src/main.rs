mod config_loader;
use config_loader::{get_config, validate_config};

#[tokio::main]
async fn main() {
    println!("Started app...");

    let config = get_config().unwrap_or_else(|err| {
        eprintln!("Failed to load config: {}", err);
        std::process::exit(1);
    });

    validate_config(&config).unwrap_or_else(|err| {
        eprintln!("Failed during config validation: {}", err);
        std::process::exit(1);
    });

    println!("Loaded config: {:?}", config);

    tokio::signal::ctrl_c().await.unwrap();
}
