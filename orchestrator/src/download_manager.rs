use std::sync::Arc;

use crate::config_loader::Settings;
use crate::util::device_jwt::create_tpm_jwt;
use crate::util::tpm::TpmSigner;
use amos_common::Page;
use amos_common::entities::reported_application_assignment::CreateModel as ReportedApplicationAssignmentCreate;
use amos_common::entities::reported_os_assignment::CreateModel as ReportedOsAssignmentCreate;
use amos_common::entities::{
    ApplicationAssignment, ApplicationConfig, ApplicationLog, DeviceLog, OsAssignment, OsVersion,
};
use anyhow::{Context, Result};
use chrono::Utc;
use futures_util::future::join_all;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, ClientBuilder};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info};

pub struct TokenState {
    pub token: String,
    pub expires_at: i64, // UTC timestamp
}

pub struct DownloadManager {
    pub http_client: RwLock<Client>,
    pub config: Arc<Settings>,
    pub signer: Arc<Mutex<TpmSigner>>,
    pub token: RwLock<TokenState>,
}

impl DownloadManager {
    pub fn new(config: Arc<Settings>, signer: TpmSigner) -> Result<Self> {
        let http_client = build_http_client(&config)?
            .build()
            .with_context(|| "Failed building HTTP client")?;

        let token_state = TokenState {
            token: String::new(),
            expires_at: Utc::now().timestamp(), // immediately expired
        };

        Ok(Self {
            http_client: RwLock::new(http_client),
            config,
            signer: Arc::new(Mutex::new(signer)),
            token: RwLock::new(token_state),
        })
    }

    async fn ensure_auth_not_expired(&self) -> Result<()> {
        // refresh 30 seconds before expiry
        let refresh_before = 30;

        {
            let state = self.token.read().await;
            if (Utc::now().timestamp() + refresh_before) < state.expires_at {
                return Ok(()); // still valid
            }
        }

        // write lock (only if expired)
        let mut state = self.token.write().await;

        // double-check after acquiring write lock
        if (Utc::now().timestamp() + refresh_before) < state.expires_at {
            return Ok(()); // someone else refreshed it
        }

        // refresh token
        debug!("Renewing device jwt");
        let mut signer = self.signer.lock().await;
        let (new_token, expiry) = create_tpm_jwt(&mut signer, self.config.device_uuid.clone())?;
        state.token = new_token;
        state.expires_at = expiry;

        // rebuild client with new default Authorization header
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", state.token))?,
        );

        let new_client = build_http_client(&self.config)?
            .default_headers(headers)
            .build()
            .with_context(|| "Failed building HTTP client")?;

        {
            let mut client = self.http_client.write().await;
            *client = new_client;
        }

        Ok(())
    }

    // Sends device pings to the API to indicate the orchestrator is still running
    pub async fn send_ping(&self) -> Result<()> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/pings/{}",
            self.config.cloud_url, self.config.device_uuid
        );

        self.http_client.read().await.put(url).send().await?;

        Ok(())
    }

    /// Fetches the OS version assigned to this device from the API.
    /// Queries `/os-assignments?device_uuid=<uuid>` then `/os-versions/<id>`.
    pub async fn get_target_os_version(&self) -> Result<OsVersion::Model> {
        self.ensure_auth_not_expired().await?;

        let assignment = self.get_target_os_assignment().await?;
        let version = self.get_os_version_by_id(assignment.os_version_id).await?;
        debug!(
            os_version_id = version.id,
            commit_hash = %version.commit_hash,
            "Resolved target OS version",
        );
        Ok(version)
    }

    async fn get_target_os_assignment(&self) -> Result<OsAssignment::Model> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/os-assignments?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );
        let resp = self
            .http_client
            .read()
            .await
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", &self.config.cloud_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Server responded with status {} for {}", resp.status(), url);
        }

        let page: Page<OsAssignment::Model> = resp
            .json()
            .await
            .with_context(|| "Failed to parse OS assignments page response")?;

        page.data.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!(
                "No OS assignment found for device {}",
                self.config.device_uuid
            )
        })
    }

    /// Reports the current OS assignment for this device to the API.
    /// POSTs to `/reported-os-assignments?device_uuid=<uuid>`.
    pub async fn report_current_os_assignment(&self, os_version_id: i32) -> Result<()> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/reported-os-assignments?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );

        let body = ReportedOsAssignmentCreate {
            os_version_id,
            device_id: None,
        };

        let resp = self
            .http_client
            .read()
            .await
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

    async fn get_os_version_by_id(&self, id: i32) -> Result<OsVersion::Model> {
        self.ensure_auth_not_expired().await?;

        let url = format!("{}/os-versions/{}", self.config.cloud_url, id);
        let resp = self
            .http_client
            .read()
            .await
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

    /// Fetches the application configs assigned to this device from the API.
    /// Resolves `/app-assignments?device_uuid=<uuid>` to the referenced
    /// `ApplicationConfig` records via `/app-configs/<id>`.
    pub async fn get_target_application_configs(&self) -> Result<Vec<ApplicationConfig::Model>> {
        self.ensure_auth_not_expired().await?;

        let assignments = self.get_target_application_assignments().await?;

        let mut fetch_futures = Vec::with_capacity(assignments.len());
        for assignment in assignments {
            fetch_futures.push(self.get_application_config_by_id(assignment.application_config_id));
        }

        let results = join_all(fetch_futures).await;

        let mut configs = Vec::with_capacity(results.len());
        for result in results {
            configs.push(result?);
        }

        debug!(count = configs.len(), "Resolved target application configs");
        Ok(configs)
    }

    async fn get_target_application_assignments(
        &self,
    ) -> Result<Vec<ApplicationAssignment::Model>> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/app-assignments?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );
        let resp = self
            .http_client
            .read()
            .await
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", &self.config.cloud_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Server responded with status {} for {}", resp.status(), url);
        }

        let page: Page<ApplicationAssignment::Model> = resp
            .json()
            .await
            .with_context(|| "Failed to parse application assignments page response")?;
        Ok(page.data)
    }

    async fn get_application_config_by_id(&self, id: i32) -> Result<ApplicationConfig::Model> {
        self.ensure_auth_not_expired().await?;

        let url = format!("{}/app-configs/{}", self.config.cloud_url, id);
        let resp = self
            .http_client
            .read()
            .await
            .get(&url)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", &self.config.cloud_url))?;

        if !resp.status().is_success() {
            anyhow::bail!("Server responded with status {} for {}", resp.status(), url);
        }

        resp.json::<ApplicationConfig::Model>()
            .await
            .with_context(|| format!("Failed to parse application config {} response", id))
    }

    /// Pushes device log entries to the API.
    /// POSTs to `/logs/devices?device_uuid=<uuid>`.
    pub async fn push_device_logs(&self, entries: Vec<DeviceLog::CreateEntry>) -> Result<()> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/logs/devices?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );

        let body = DeviceLog::CreateModel { entries };

        let resp = self
            .http_client
            .read()
            .await
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

    /// Pushes application log entries for a given application to the API.
    /// POSTs to `/logs/applications?device_uuid=<uuid>`.
    pub async fn push_application_logs(
        &self,
        application_id: i32,
        entries: Vec<ApplicationLog::CreateEntry>,
    ) -> Result<()> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/logs/applications?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );

        let body = ApplicationLog::CreateModel {
            application_id,
            entries,
        };

        let resp = self
            .http_client
            .read()
            .await
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

    /// Reports the current running application config for this device to the API.
    /// POSTs to `/reported-app-assignments?device_uuid=<uuid>`.
    pub async fn report_current_application_assignment(
        &self,
        application_config_id: i32,
    ) -> Result<()> {
        self.ensure_auth_not_expired().await?;

        let url = format!(
            "{}/reported-app-assignments?device_uuid={}",
            self.config.cloud_url, self.config.device_uuid
        );

        let body = ReportedApplicationAssignmentCreate {
            application_config_id,
            device_id: None,
        };

        let resp = self
            .http_client
            .read()
            .await
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
}

// If https_proxy is set, it will be used for all requests. Otherwise, reqwest will use https_proxy from the environment variables.
fn build_http_client(settings: &Settings) -> Result<ClientBuilder> {
    let mut builder = Client::builder();

    if let Some(proxy_url) = &settings.https_proxy {
        info!("Using https proxy: {}", proxy_url);
        let proxy = reqwest::Proxy::https(proxy_url)
            .with_context(|| format!("Failed to set https proxy: {}", proxy_url))?;
        builder = builder.proxy(proxy);
    } else {
        info!("No https proxy set, using environment variables if available");
    }

    Ok(builder)
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
