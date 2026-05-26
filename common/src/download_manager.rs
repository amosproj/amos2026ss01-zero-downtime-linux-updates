use crate::api::{CatalogResponse, CatalogResponseEntry};
use crate::inventory_model::SystemRequirements;
use crate::util::Base64;
use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

// Temporary config struct containing server URL https_proxy, and a path to which updates will be downloaded.
// This config should not be necessary in the future, when the orchestrator will provide the download manager with the config or its relevant values.
pub struct Config {
    pub server_url: String,
    pub https_proxy: Option<String>,
    pub download_dir: PathBuf,
}

// Temporary structs for holding (and returning) the server response with owned data since the current CatalogResponseEntry struct contains borrowed data
// This struct should not be necessary in the future, when it is clear what logic the check_for_update function will need to implement
// e.g. checking version discrepancies to currently installed os and apps and only returning relevant update information instead of the entire server response.
pub struct CatalogResponseEntryWithOwnedData {
    pub name: String,
    pub version: String,
    pub url: String,
    pub signature: Base64<'static>,
}
pub struct CatalogResponseWithOwnedData {
    pub entries: Vec<CatalogResponseEntryWithOwnedData>,
}

// Builds the async reqwest HTTP client.
// If `https_proxy` is set, it is used for all requests. Otherwise, reqwest falls back to the
// https_proxy environment variable if set.
pub fn build_http_client(https_proxy: Option<&str>) -> Result<Client> {
    let mut builder = Client::builder();

    if let Some(proxy_url) = https_proxy {
        println!("Using https proxy: {}", proxy_url);
        let proxy = reqwest::Proxy::https(proxy_url)
            .with_context(|| format!("Failed to set https proxy: {}", proxy_url))?;
        builder = builder.proxy(proxy);
    } else {
        println!("No https proxy set, using environment variables if available");
    }

    builder
        .build()
        .with_context(|| "Failed building HTTP client")
}

// Poll the server to check what the available OS version is
// Exact details of logic wanted here are still tbd, so for now this function returns a owned version of the entire server response.
// Returns:
// - Ok(CatalogResponseWithOwnedData) if the server responded with a valid catalog response
// - Err(anyhow::Error) if there was an error making the request or parsing the response, with a context message indicating the failure reason
pub async fn check_for_update(
    client: &Client,
    config: &Config,
) -> Result<CatalogResponseWithOwnedData> {
    // Await server response with hard coded timeout. Could be added to config in the future.
    let resp = client
        .get(format!("{}/v1/catalog", config.server_url))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("Failed to reach server at {}", &config.server_url))?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Server at {} responded with status code: {}",
            &config.server_url,
            resp.status()
        );
    }

    let text = resp
        .text()
        .await
        .with_context(|| "Failed to read server response as text")?;

    let update_info: CatalogResponse = serde_json::from_str::<CatalogResponse>(&text)
        .with_context(|| "Failed to parse server response as CatalogResponse")?;

    // Temporary conversion to struct with owned data, should not be necessary in the future when the logic of what to do with the
    // CatalogResponse is implemented and we can process the relevant information instead of the entire server response.
    let owned_update_info = CatalogResponseWithOwnedData {
        entries: update_info
            .0
            .iter()
            .map(|entry| CatalogResponseEntryWithOwnedData {
                name: entry.name.to_string(),
                version: entry.version.to_string(),
                url: entry.url.to_string(),
                signature: Base64::from(entry.signature.0.clone().into_owned()),
            })
            .collect(),
    };

    // Perhaps we shouldn't return what is essentially the entire server api response, but the specifics of what should happen
    // (e.g. checking version discrepancies to currently installed os and apps) instead is a problem for future us.
    Ok(owned_update_info)
}

// Fetches the cloud-side target system requirements (Device Inventory MVP shape).
// Returns:
// - Ok(SystemRequirements) if the server responded with a valid requirements payload
// - Err(anyhow::Error) on transport, status, or deserialization failures, with context.
pub async fn get_system_requirements(
    client: &Client,
    server_url: &str,
) -> Result<SystemRequirements> {
    let url = format!("{}/v1/system-requirements", server_url);

    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("Failed to reach server at {}", url))?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Server at {} responded with status code: {}",
            url,
            resp.status()
        );
    }

    resp.json::<SystemRequirements>()
        .await
        .with_context(|| "Failed to parse server response as SystemRequirements")
}

// Downloads the update from a given CatalogResponseEntry and saves it as "update_<name>_<version>.bin" to the directory specified in config.download_dir.
// Input will most likely not be a CatalogResponseEntry in the future due to temporary nature of the current CatalogResponseEntry struct.
// Returns:
// - Ok(PathBuf) with the path to the downloaded file if the download was successful
// - Err(anyhow::Error) if there was an error during the download or file writing process, with a context message indicating the failure reason
pub async fn download_update(
    client: &Client,
    single_update_info: &CatalogResponseEntry<'static>,
    config: &Config,
) -> Result<PathBuf> {
    // Creates download directory according to config, which currently ONLY EXISTS IN THIS TEMPORARY CONFIG STRUCT
    tokio::fs::create_dir_all(&config.download_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to create download directory at {:?}",
                &config.download_dir
            )
        })?;

    let filename = format!(
        "update_{}_{}.bin",
        single_update_info.name, single_update_info.version
    );
    let file_path = config.download_dir.join(&filename);

    // Await server download response with hard coded (long) timeout. Could be added to config in the future.
    let resp = client
        .get(single_update_info.url)
        .timeout(std::time::Duration::from_secs(3600)) // Potentially long download, timeout set to 1 hour for now
        .send()
        .await
        .with_context(|| format!("Failed to download update from {}", &single_update_info.url))?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to download update, server responded with status code: {}",
            resp.status()
        );
    }

    // Create the file to write the downloaded update into
    let mut file = tokio::fs::File::create(&file_path)
        .await
        .with_context(|| format!("Failed to create file at {:?}", &file_path))?;

    // Get the response body as a stream of bytes to write it in chunks, which is more efficient for large files
    let mut stream = resp.bytes_stream();

    // Write the download in chunks
    while let Some(chunk) = stream.next().await {
        let dl_chunk = chunk.with_context(|| "Failed to read chunk from download stream")?;
        file.write_all(&dl_chunk)
            .await
            .with_context(|| "Failed to write chunk to file")?;
    }

    Ok(file_path)
}
