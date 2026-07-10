use amos_common::Page;
use amos_common::entities::{Device, LogEvent, LogKind, LogLevel, LogStreamQuery};
use eventsource_stream::Eventsource;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;

/// Resolved connection config: user base URL (incl. `/v1`) + bearer JWT.
#[derive(Clone)]
pub struct Config {
    pub base_url: String,
    pub jwt: String,
}

/// A message from a stream task to the UI. `epoch` identifies which
/// subscription produced it, so the app can discard messages from a stream it
/// has already torn down (e.g. after changing the filter).
pub struct Msg {
    pub epoch: u64,
    pub kind: MsgKind,
}

pub enum MsgKind {
    Connected,
    Log(Box<LogEvent>),
    Disconnected(String),
    ParseWarn(String),
}

/// Handle to a running stream task. Dropping or calling [`stop`] aborts it,
/// which drops the streaming response body and closes the TCP connection.
pub struct StreamHandle(AbortHandle);

impl StreamHandle {
    pub fn stop(self) {
        self.0.abort();
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        self.0.abort();
    }
}

#[derive(Clone)]
pub struct Client {
    http: reqwest::Client,
    cfg: Config,
}

impl Client {
    pub fn new(cfg: Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            cfg,
        }
    }

    /// `GET /devices` — one page big enough for the demo fleet.
    pub async fn fetch_devices(&self) -> anyhow::Result<Vec<Device::Model>> {
        let page: Page<Device::Model> = self
            .http
            .get(format!("{}/devices?page_size=100", self.cfg.base_url))
            .bearer_auth(&self.cfg.jwt)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(page.data)
    }

    /// Spawn an SSE task for `GET /logs/stream` with the given filters. The task
    /// pushes [`Msg`]s tagged with `epoch` until aborted via the returned handle.
    pub fn spawn_stream(
        &self,
        query: LogStreamQuery,
        epoch: u64,
        tx: mpsc::Sender<Msg>,
    ) -> StreamHandle {
        let http = self.http.clone();
        let base = self.cfg.base_url.clone();
        let jwt = self.cfg.jwt.clone();
        let handle = tokio::spawn(async move {
            stream_task(http, base, jwt, query, epoch, tx).await;
        });
        StreamHandle(handle.abort_handle())
    }
}

async fn stream_task(
    http: reqwest::Client,
    base: String,
    jwt: String,
    query: LogStreamQuery,
    epoch: u64,
    tx: mpsc::Sender<Msg>,
) {
    // Values are simple ints / lowercase words, so no URL-encoding is needed.
    let mut params: Vec<String> = Vec::new();
    if let Some(level) = query.level {
        params.push(format!("level={}", level_str(level)));
    }
    if let Some(device_id) = query.device_id {
        params.push(format!("device_id={device_id}"));
    }
    if let Some(application_id) = query.application_id {
        params.push(format!("application_id={application_id}"));
    }
    if let Some(kind) = query.kind {
        params.push(format!("kind={}", kind_str(kind)));
    }
    let url = if params.is_empty() {
        format!("{base}/logs/stream")
    } else {
        format!("{base}/logs/stream?{}", params.join("&"))
    };

    let resp = match http
        .get(url)
        .bearer_auth(&jwt)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(r) => r,
        Err(e) => {
            let _ = tx
                .send(Msg {
                    epoch,
                    kind: MsgKind::Disconnected(e.to_string()),
                })
                .await;
            return;
        }
    };

    let _ = tx
        .send(Msg {
            epoch,
            kind: MsgKind::Connected,
        })
        .await;

    let mut stream = resp.bytes_stream().eventsource();
    while let Some(item) = stream.next().await {
        match item {
            Ok(ev) => {
                // Keep-alive / comment events carry no data — skip them.
                if ev.data.is_empty() {
                    continue;
                }
                // `LogEvent` is internally tagged (`{"kind":"device", ...}`),
                // so each `data:` payload deserializes directly.
                match serde_json::from_str::<LogEvent>(&ev.data) {
                    Ok(log) => {
                        let msg = Msg {
                            epoch,
                            kind: MsgKind::Log(Box::new(log)),
                        };
                        if tx.send(msg).await.is_err() {
                            break; // UI dropped the receiver
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(Msg {
                                epoch,
                                kind: MsgKind::ParseWarn(e.to_string()),
                            })
                            .await;
                    }
                }
            }
            Err(e) => {
                let _ = tx
                    .send(Msg {
                        epoch,
                        kind: MsgKind::Disconnected(e.to_string()),
                    })
                    .await;
                break;
            }
        }
    }
}

/// On-wire lowercase form of a level (matches `serde(rename_all = "lowercase")`).
pub fn level_str(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Trace => "trace",
        LogLevel::Debug => "debug",
        LogLevel::Info => "info",
        LogLevel::Warn => "warn",
        LogLevel::Error => "error",
        LogLevel::Fatal => "fatal",
    }
}

fn kind_str(kind: LogKind) -> &'static str {
    match kind {
        LogKind::Device => "device",
        LogKind::Application => "application",
    }
}
