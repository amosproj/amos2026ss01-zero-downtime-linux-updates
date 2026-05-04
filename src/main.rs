mod config_loader;
use config_loader::get_config;
mod state;
use std::process::exit;

#[tokio::main]
async fn main() {
    println!("Started app...");

    let cfg = match get_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            exit(1);
        }
    };
    println!("Loaded config...");
    println!("Got config: {:?}", cfg);

    tokio::signal::ctrl_c().await.unwrap();
}
