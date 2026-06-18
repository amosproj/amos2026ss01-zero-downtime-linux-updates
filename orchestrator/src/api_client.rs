use std::time::Duration;

use crate::util::device_jwt::DeviceJwtProvider;
use amos_common::Page;
use amos_common::entities::reported_application_assignment::CreateModel as ReportedApplicationAssignmentCreate;
use amos_common::entities::reported_os_assignment::CreateModel as ReportedOsAssignmentCreate;
use amos_common::entities::{
    ApplicationAssignment, ApplicationConfig, ApplicationLog, DeviceLog, OsAssignment, OsVersion,
};
use anyhow::{Context, Result};
use reqwest::Method;
use serde::Serialize;
use tracing::{debug, info};

pub struct TokenState {
    pub token: String,
    pub expires_at: i64, // UTC timestamp
}

pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    device_uuid: String,
    jwt_provider: tokio::sync::Mutex<DeviceJwtProvider>,
}

impl ApiClient {
    pub fn new(
        proxy: Option<String>,
        base_url: String,
        device_uuid: String,
        jwt_provider: DeviceJwtProvider,
    ) -> anyhow::Result<Self> {
        let mut cb = reqwest::ClientBuilder::new();

        // Set proxy from config if necessary
        if let Some(proxy_url) = proxy {
            info!("Using https proxy: {}", proxy_url);
            let proxy = match proxy_url {
                http_proxy if http_proxy.starts_with("http://") => {
                    reqwest::Proxy::http(http_proxy)?
                }
                https_proxy if https_proxy.starts_with("https://") => {
                    reqwest::Proxy::https(https_proxy)?
                }
                _ => anyhow::bail!("Unknown proxy URL scheme"),
            };
            cb = cb.proxy(proxy);
        } else {
            debug!("No proxy set, using environment variables if available");
        }

        cb = cb.timeout(Duration::from_secs(30));

        Ok(Self {
            client: cb.build()?,
            base_url,
            device_uuid,
            jwt_provider: tokio::sync::Mutex::const_new(jwt_provider),
        })
    }

    // Sends device pings to the API to indicate the orchestrator is still running
    pub async fn send_ping(&self) -> anyhow::Result<()> {
        self.put(&format!("/pings/{}", self.device_uuid)).await
    }

    /// Fetches the OS version assigned to this device from the API.
    /// Queries `/os-assignments?device_uuid=<uuid>` then `/os-versions/<id>`.
    pub async fn get_target_os_version(&self) -> Result<OsVersion::Model> {
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
        let page: Page<OsAssignment::Model> = self
            .get(&format!("/os-assignments?device_uuid={}", self.device_uuid))
            .await?;

        page.data.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!("No OS assignment found for device {}", self.device_uuid)
        })
    }

    /// Reports the current OS assignment for this device to the API.
    /// POSTs to `/reported-os-assignments?device_uuid=<uuid>`.
    pub async fn report_current_os_assignment(&self, os_version_id: i32) -> Result<()> {
        self.post(
            &format!("/reported-os-assignments?device_uuid={}", self.device_uuid),
            ReportedOsAssignmentCreate {
                os_version_id,
                device_id: None,
            },
        )
        .await
    }

    async fn get_os_version_by_id(&self, id: i32) -> Result<OsVersion::Model> {
        self.get(&format!("/os-versions/{}", id)).await
    }

    /// Fetches the application configs assigned to this device from the API.
    /// Resolves `/app-assignments?device_uuid=<uuid>` to the referenced
    /// `ApplicationConfig` records via `/app-configs/<id>`.
    pub async fn get_target_application_configs(&self) -> Result<Vec<ApplicationConfig::Model>> {
        let assignments = self.get_target_application_assignments().await?;

        let app_conf_results = futures_util::future::join_all(
            assignments
                .into_iter()
                .map(|a| self.get_application_config_by_id(a.application_config_id)),
        )
        .await;

        Ok(app_conf_results.into_iter().collect::<Result<Vec<_>>>()?)
    }

    async fn get_target_application_assignments(
        &self,
    ) -> Result<Vec<ApplicationAssignment::Model>> {
        self.get(&format!(
            "/app-assignments?device_uuid={}",
            self.device_uuid
        ))
        .await
    }

    async fn get_application_config_by_id(&self, id: i32) -> Result<ApplicationConfig::Model> {
        self.get(&format!("/app-configs/{}", id)).await
    }

    /// Pushes device log entries to the API.
    /// POSTs to `/logs/devices?device_uuid=<uuid>`.
    pub async fn push_device_logs(&self, entries: Vec<DeviceLog::CreateEntry>) -> Result<()> {
        self.post(
            &format!("/logs/devices?device_uuid={}", self.device_uuid),
            DeviceLog::CreateModel { entries },
        )
        .await
    }

    #[allow(dead_code)] // TODO: remove when implementing podman application logs
    /// Pushes application log entries for a given application to the API.
    /// POSTs to `/logs/applications?device_uuid=<uuid>`.
    pub async fn push_application_logs(
        &self,
        application_id: i32,
        entries: Vec<ApplicationLog::CreateEntry>,
    ) -> Result<()> {
        self.post(
            &format!("/logs/applications?device_uuid={}", self.device_uuid),
            ApplicationLog::CreateModel {
                application_id,
                entries,
            },
        )
        .await
    }

    /// Reports the current running application config for this device to the API.
    /// POSTs to `/reported-app-assignments?device_uuid=<uuid>`.
    pub async fn report_current_application_assignment(
        &self,
        application_config_id: i32,
    ) -> Result<()> {
        self.post(
            &format!("/reported-app-assignments?device_uuid={}", self.device_uuid),
            ReportedApplicationAssignmentCreate {
                application_config_id,
                device_id: None,
            },
        )
        .await
    }

    // -- Internal helper functions --
    async fn req(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<impl Serialize>,
    ) -> anyhow::Result<reqwest::Response> {
        let url = format!("{}{}", self.base_url, endpoint);
        let auth_header = {
            let mut provider = self.jwt_provider.lock().await;
            format!("Bearer {}", provider.token(&self.device_uuid)?)
        };

        let mut req = self
            .client
            .request(method, url)
            .header(reqwest::header::AUTHORIZATION, auth_header);

        if let Some(b) = body {
            req = req.json(&b)
        }

        let res = req
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", self.base_url))?;

        if !res.status().is_success() {
            anyhow::bail!(
                "Server responded with status {} for {}",
                res.status(),
                endpoint
            );
        }

        Ok(res)
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, endpoint: &str) -> anyhow::Result<T> {
        let res = self.req(Method::GET, endpoint, None as Option<()>).await?;
        res.json()
            .await
            .with_context(|| format!("Unexpected response from {}", endpoint))
    }

    async fn post(&self, endpoint: &str, body: impl Serialize) -> anyhow::Result<()> {
        self.req(Method::POST, endpoint, Some(body)).await?;
        Ok(())
    }

    async fn put(&self, endpoint: &str) -> anyhow::Result<()> {
        self.req(Method::PUT, endpoint, None as Option<()>).await?;
        Ok(())
    }
}
