//! Interact with the Podman socket

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::stream::BoxStream;

pub mod log_registry;
pub mod wrapper;

/// Podman label used to recover an application's id from a running container
pub const LABEL_APP_ID: &str = "org.amos.application_id";
/// Podman label used to recover the application_config id a container was
/// launched from, so config-only changes can be detected on reconcile
pub const LABEL_APP_CONFIG_ID: &str = "org.amos.application_config_id";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogStreamKind {
    Stdout,
    Stderr,
}

#[derive(Clone, Debug)]
pub struct LogChunk {
    pub stream: LogStreamKind,
    pub time: Option<DateTime<Utc>>,
    pub message: String,
}

pub trait PodmanLogHandle: Send + 'static {
    fn logs(
        self,
        follow: bool,
        since: Option<DateTime<Utc>>,
    ) -> BoxStream<'static, anyhow::Result<LogChunk>>;
}

#[async_trait]
pub trait Podman: 'static {
    type PImage<'a>: PodmanImage
    where
        Self: 'a;
    type PContainer: PodmanContainer;

    async fn image<'a>(
        &'a self,
        reference: &str,
        behaviour: PodmanPullBehaviour,
    ) -> anyhow::Result<Option<Self::PImage<'a>>>;

    async fn prune_images(&mut self) -> anyhow::Result<()>;
}

pub trait PodmanImageInfo {
    /// Reference the image was tagged as, e.g. "docker.io/library/alpine:latest"
    fn reference(&self) -> &str;

    /// Unique fingerprint of the image, usually a SHA256
    fn digest(&self) -> &str;

    /// The application_config id this was created from/for, if known.
    /// Used to detect config-only changes on reconcile.
    fn application_config_id(&self) -> Option<i32> {
        None
    }
}

#[async_trait]
pub trait PodmanImage: PodmanImageInfo + Send {
    type PContainer: PodmanContainer;

    async fn create_container(
        &self,
        name: &str,
        config: Option<amos_common::entities::ContainerConfigV1>,
        application_id: i32,
        application_config_id: i32,
    ) -> anyhow::Result<Self::PContainer>;
}

#[async_trait]
pub trait PodmanContainer: PodmanImageInfo + Send + 'static {
    type LogHandle: PodmanLogHandle;

    async fn start(&mut self) -> anyhow::Result<()>;
    #[allow(dead_code)]
    async fn stop(&mut self) -> anyhow::Result<()>;
    async fn destroy(self) -> anyhow::Result<()>;
    async fn state(&self) -> anyhow::Result<PodmanContainerState>;
    async fn wait_for_state_change(&self, current: PodmanContainerState) -> anyhow::Result<()>;
    fn name(&self) -> &str;
    fn take_log_handle(&mut self) -> Option<Self::LogHandle>;

    /// The application id this container was launched for, if known
    /// (recovered from a podman label on startup).
    fn application_id(&self) -> Option<i32> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[expect(unused)]
pub enum PodmanPullBehaviour {
    AlwaysPull,
    PullIfMissingOrNewer,
    PullIfMissing,
    NeverPull,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodmanContainerState {
    Stopped,
    Ambiguous,
    Running,
}
