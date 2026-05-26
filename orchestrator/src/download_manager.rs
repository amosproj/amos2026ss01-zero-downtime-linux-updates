use anyhow::{Context, Result};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

use crate::config_loader::Settings;

pub struct DownloadManagerConfig {
    pub server_url: String,
    pub https_proxy: Option<String>,
    pub download_dir: PathBuf,
    pub device_uuid: String,
}

#[derive(Serialize)]
pub struct DeviceSyncRequest {
    pub device_uuid: String,
    pub current_os_version: String,
}

#[derive(Debug, Deserialize)]
pub struct CloudSyncResponse {
    pub target_os_commit_hash: Option<String>,
    pub orchestrator_version: Option<String>,
    pub description: Option<String>,
}

/// Creates a DownloadManager HTTP client from the application Settings.
/// `current_os_version` is passed separately because it comes from runtime state, not config.
pub fn build_http_client_from_settings(settings: &Settings) -> Result<Client> {
    let config = DownloadManagerConfig {
        server_url: settings.cloud_url.clone(),
        https_proxy: settings.https_proxy.clone(),
        download_dir: PathBuf::from(&settings.download_dir),
        device_uuid: settings.device_uuid.clone(),
    };
    build_http_client(&config)
}

// Builds the async reqwest HTTP client
// If https_proxy is set, it will be used for all requests. Otherwise, reqwest will use https_proxy from the environment variables.
pub fn build_http_client(config: &DownloadManagerConfig) -> Result<Client> {
    let mut builder = Client::builder();

    if let Some(proxy_url) = &config.https_proxy {
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
pub async fn check_for_update(
    client: &Client,
    config: &DownloadManagerConfig,
) -> Result<CloudSyncResponse> {
    let request_payload = DeviceSyncRequest {
        device_uuid: config.device_uuid.clone(),
        current_os_version: config.current_os_version.clone(),
    };

    let resp = client
        .post(format!("{}/v1/devices/sync", config.server_url))
        .json(&request_payload)
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

    let update_info: CloudSyncResponse = serde_json::from_str(&text)
        .with_context(|| "Failed to parse server response as CloudSyncResponse")?;

    Ok(update_info)
}

// Downloads the update from the server based on the target commit hash
pub async fn download_update(
    client: &Client,
    target_commit_hash: &str,
    config: &DownloadManagerConfig,
) -> Result<PathBuf> {
    tokio::fs::create_dir_all(&config.download_dir)
        .await
        .with_context(|| {
            format!(
                "Failed to create download directory at {:?}",
                &config.download_dir
            )
        })?;

    let filename = format!("update_{}.bin", target_commit_hash);
    let file_path = config.download_dir.join(&filename);

    // Need to configure bootc download here for mid project review
    let download_url = format!("{}/v1/download/{}", config.server_url, target_commit_hash);

    let resp = client
        .get(&download_url)
        .timeout(std::time::Duration::from_secs(3600))
        .send()
        .await
        .with_context(|| format!("Failed to download update from {}", &download_url))?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to download update, server responded with status code: {}",
            resp.status()
        );
    }

    let mut file = tokio::fs::File::create(&file_path)
        .await
        .with_context(|| format!("Failed to create file at {:?}", &file_path))?;

    // Get the response body as a stream of bytes to write it in chunks, which is more efficient for large files
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let dl_chunk = chunk.with_context(|| "Failed to read chunk from download stream")?;
        file.write_all(&dl_chunk)
            .await
            .with_context(|| "Failed to write chunk to file")?;
    }

    Ok(file_path)
}
