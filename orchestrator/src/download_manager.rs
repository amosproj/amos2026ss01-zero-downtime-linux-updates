use amos_common::entities::{OsAssignment, OsVersion, ReportedOsAssignment};
use anyhow::{Context, Result};
use log::{info, warn};
use reqwest::Client;
use std::sync::Arc;

use crate::config_loader::Settings;

pub struct DownloadManager {
    pub http_client: Client,
    pub config: Arc<Settings>,
}

impl DownloadManager {
    pub fn new(config: Arc<Settings>) -> Result<Self> {
        let http_client = build_http_client(&config)?;
        Ok(Self {
            http_client,
            config,
        })
    }

    // Sends device pings to the API to indicate the orchestrator is still running
    pub async fn send_ping(&self) {
        let url = format!(
            "{}/pings/{}",
            self.config.cloud_url, self.config.device_uuid
        );

        let result = self.http_client.put(url).send().await;
        if let Err(err) = result {
            warn!("Aliveness report failed: {}", err);
        }
    }

    /// Fetches the OS version assigned to this device from the API.
    /// Queries `/os-assignments?device_uuid=<uuid>` then `/os-versions/<id>`.
    pub async fn get_expected_os_version(&self) -> Result<OsVersion::Model> {
        let assignment = self.get_os_assignment().await?;
        self.get_os_version(assignment.os_version_id).await
    }

    async fn get_os_assignment(&self) -> Result<OsAssignment::Model> {
        let url = format!(
            "{}/os-assignments?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );
        let resp = self
            .http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", &self.config.cloud_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Server responded with status {} for {}", resp.status(), url);
        }

        let assignments: Vec<OsAssignment::Model> = resp
            .json()
            .await
            .with_context(|| "Failed to parse OS assignments response")?;

        assignments.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!(
                "No OS assignment found for device {}",
                self.config.device_uuid
            )
        })
    }

    /// Reports the current OS assignment for this device to the API.
    /// POSTs to `/reported-os-assignments?device_uuid=<uuid>`.
    pub async fn report_os_assignment(&self, os_version_id: i32) -> Result<()> {
        let url = format!(
            "{}/reported-os-assignments?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );

        let body = ReportedOsAssignment::Model {
            id: 0,
            os_version_id,
            device_id: 0,
            updated_at: chrono::Utc::now(),
        };

        let resp = self
            .http_client
            .post(&url)
            .json(&body)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", &self.config.cloud_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Server responded with status {} for {}", resp.status(), url);
        }

        Ok(())
    }

    async fn get_os_version(&self, id: i32) -> Result<OsVersion::Model> {
        let url = format!("{}/os-versions/{}", self.config.cloud_url, id);
        let resp = self
            .http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", &self.config.cloud_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Server responded with status {} for {}", resp.status(), url);
        }

        resp.json::<OsVersion::Model>()
            .await
            .with_context(|| format!("Failed to parse OS version {} response", id))
    }
}

// If https_proxy is set, it will be used for all requests. Otherwise, reqwest will use https_proxy from the environment variables.
fn build_http_client(settings: &Settings) -> Result<Client> {
    let mut builder = Client::builder();

    if let Some(proxy_url) = &settings.https_proxy {
        info!("Using https proxy: {}", proxy_url);
        let proxy = reqwest::Proxy::https(proxy_url)
            .with_context(|| format!("Failed to set https proxy: {}", proxy_url))?;
        builder = builder.proxy(proxy);
    } else {
        info!("No https proxy set, using environment variables if available");
    }

    builder
        .build()
        .with_context(|| "Failed building HTTP client")
}

// // Poll the server to check what the available OS version is
// pub async fn check_for_update(
//     client: &Client,
//     config: &Arc<Settings>,
// ) -> Result<CloudSyncResponse> {
//     let request_payload = DeviceSyncRequest {
//         device_uuid: config.device_uuid.clone(),
//         current_os_version: config.current_os_version.clone(),
//     };
//
//     let resp = client
//         .post(format!("{}/v1/devices/sync", config.cloud_url))
//         .json(&request_payload)
//         .timeout(std::time::Duration::from_secs(10))
//         .send()
//         .await
//         .with_context(|| format!("Failed to reach server at {}", &config.cloud_url))?;
//
//     if !resp.status().is_success() {
//         anyhow::bail!(
//             "Server at {} responded with status code: {}",
//             &config.cloud_url,
//             resp.status()
//         );
//     }
//
//     let text = resp
//         .text()
//         .await
//         .with_context(|| "Failed to read server response as text")?;
//
//     let update_info: CloudSyncResponse = serde_json::from_str(&text)
//         .with_context(|| "Failed to parse server response as CloudSyncResponse")?;
//
//     Ok(update_info)
// }
//
// // Downloads the update from the server based on the target commit hash
// pub async fn download_update(
//     client: &Client,
//     target_commit_hash: &str,
//     config: &Arc<Settings>,
// ) -> Result<PathBuf> {
//     tokio::fs::create_dir_all(&config.download_dir)
//         .await
//         .with_context(|| {
//             format!(
//                 "Failed to create download directory at {:?}",
//                 &config.download_dir
//             )
//         })?;
//
//     let filename = format!("update_{}.bin", target_commit_hash);
//     let file_path = PathBuf::from(&config.download_dir).join(&filename);
//
//     let download_url = format!("{}/v1/download/{}", config.cloud_url, target_commit_hash);
//
//     let resp = client
//         .get(&download_url)
//         .timeout(std::time::Duration::from_secs(3600))
//         .send()
//         .await
//         .with_context(|| format!("Failed to download update from {}", &download_url))?;
//
//     if !resp.status().is_success() {
//         anyhow::bail!(
//             "Failed to download update, server responded with status code: {}",
//             resp.status()
//         );
//     }
//
//     let mut file = tokio::fs::File::create(&file_path)
//         .await
//         .with_context(|| format!("Failed to create file at {:?}", &file_path))?;
//
//     let mut stream = resp.bytes_stream();
//
//     while let Some(chunk) = stream.next().await {
//         let dl_chunk = chunk.with_context(|| "Failed to read chunk from download stream")?;
//         file.write_all(&dl_chunk)
//             .await
//             .with_context(|| "Failed to write chunk to file")?;
//     }
//
//     Ok(file_path)
// }
