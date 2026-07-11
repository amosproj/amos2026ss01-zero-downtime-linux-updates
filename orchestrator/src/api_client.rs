//! Type-safe API client

use std::time::Duration;

use crate::util::device_jwt::DeviceJwtProvider;
use anyhow::{Context, Result};
use reqwest::{Method, StatusCode};
use serde::Serialize;

/// Type-safe API client.
/// Handles proxies, base urls, authentication and filtering for the current device.
pub struct ApiClient {
    client: reqwest::Client,
    base_url: String,
    device_uuid: String,
    serial_number: String,
    jwt_provider: tokio::sync::Mutex<DeviceJwtProvider>,
}

impl ApiClient {
    pub fn new(
        proxy: Option<String>,
        base_url: String,
        device_uuid: String,
        serial_number: String,
        jwt_provider: DeviceJwtProvider,
    ) -> anyhow::Result<Self> {
        let mut cb = reqwest::ClientBuilder::new();

        // Set proxy from config if necessary
        if let Some(proxy_url) = proxy {
            tracing::info!("Using https proxy: {}", proxy_url);
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
            tracing::info!("No proxy set, using environment variables if available");
        }

        let client = cb.timeout(Duration::from_secs(30)).build()?;

        Ok(Self {
            client,
            base_url,
            device_uuid,
            serial_number,
            jwt_provider: tokio::sync::Mutex::const_new(jwt_provider),
        })
    }

    // Sends device pings to the API to indicate the orchestrator is still running
    pub async fn send_ping(&self, uptime_secs: Option<i64>) -> anyhow::Result<()> {
        let body =
            uptime_secs.map(|uptime_secs| amos_common::device_api::ping::PutBody { uptime_secs });
        self.put("/device/ping", body).await
    }

    /// Fetches the OS version assigned to this device from the API
    pub async fn get_target_os_version(
        &self,
    ) -> anyhow::Result<amos_common::device_api::os::GetResponse> {
        self.get("/device/os").await
    }

    /// Reports the current OS assignment for this device to the API
    pub async fn report_current_os_assignment(&self, os_version_id: i32) -> anyhow::Result<()> {
        self.put(
            "/device/os",
            Some(amos_common::device_api::os::PutBody { os_version_id }),
        )
        .await
    }

    /// Fetches the application configs assigned to this device from the API
    pub async fn get_target_application_configs(
        &self,
    ) -> anyhow::Result<amos_common::device_api::apps::GetResponse> {
        self.get("/device/apps").await
    }

    /// Reports the current running application config for this device to the API
    pub async fn report_current_application_assignment(
        &self,
        application_config_ids: impl Iterator<Item = i32>,
    ) -> Result<()> {
        let entries: Vec<_> = application_config_ids
            .map(|id| amos_common::device_api::apps::PutBodyItem {
                application_config_id: id,
            })
            .collect();

        self.put("/device/apps", Some(entries)).await
    }

    /// Pushes device log entries to the API
    pub async fn push_device_logs(
        &self,
        entries: &[amos_common::device_api::logs::PostBodyItem],
    ) -> Result<()> {
        self.post("/device/logs", entries).await
    }

    /// Pushes application log entries for a given application to the API
    pub async fn push_application_logs(
        &self,
        application_id: i32,
        entries: &[amos_common::device_api::logs::PostBodyItem],
    ) -> Result<()> {
        self.post(
            &format!("/device/logs?application_id={}", application_id),
            entries,
        )
        .await
    }

    /// Registers the device at the API. Usually called when another response indicates
    /// that the device is unknown to the API.
    async fn register_self(&self) -> Result<()> {
        let registration_payload = {
            let mut provider = self.jwt_provider.lock().await;
            let endorsement_pubkey = provider.get_endorsement_key()?;
            let signing_pubkey = provider.get_signing_key()?;

            amos_common::device_api::register::PostBody {
                uuid: self.device_uuid.clone(),
                serial_number: self.serial_number.clone(),
                endorsement_public_key: endorsement_pubkey,
                signing_public_key: signing_pubkey,
            }
        };

        // NOTE: The .req wrapper is explicitly NOT used to avoid recursion
        let url = format!("{}/register", self.base_url);
        let req = self
            .client
            .request(Method::POST, url)
            .json(&registration_payload);

        let res = req
            .send()
            .await
            .with_context(|| format!("Failed to reach server at {}", self.base_url))?;

        if !res.status().is_success() {
            anyhow::bail!(
                "Self-registration did not succeed, server responded {}",
                res.status()
            );
        }
        tracing::info!("Successfully self-registered device");

        Ok(())
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
            .context(format!("Failed to reach server at {}", self.base_url))?;

        if res.status() == StatusCode::IM_A_TEAPOT {
            tracing::warn!(
                "Server indicated that it doesn't know this device, trying self-registration"
            );
            self.register_self().await?;
        }

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
            .context(format!("Unexpected response from {}", endpoint))
    }

    async fn post(&self, endpoint: &str, body: impl Serialize) -> anyhow::Result<()> {
        self.req(Method::POST, endpoint, Some(body)).await?;
        Ok(())
    }

    async fn put(&self, endpoint: &str, body: Option<impl Serialize>) -> anyhow::Result<()> {
        self.req(Method::PUT, endpoint, body).await?;
        Ok(())
    }
}
