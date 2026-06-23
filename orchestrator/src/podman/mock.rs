use amos_common::entities::ContainerConfigV1;
// Mock Podman for testing purposes
use chrono::{DateTime, Utc};
use futures_util::stream::{self, BoxStream, StreamExt as _};

use super::{LogChunk, LogStreamKind, PodmanLogHandle};

use std::time::Duration;

use async_trait::async_trait;

pub struct PodmanMock;
pub struct PodmanMockImage {
    reference: String,
}
pub struct PodmanMockContainer {
    name: String,
    reference: String,
    state: super::PodmanContainerState,
}

pub struct PodmanMockLogHandle {
    name: String,
}

impl PodmanLogHandle for PodmanMockLogHandle {
    fn logs(
        self,
        follow: bool,
        _since: Option<DateTime<Utc>>,
    ) -> BoxStream<'static, anyhow::Result<LogChunk>> {
        let canned = vec![
            Ok(LogChunk {
                stream: LogStreamKind::Stdout,
                time: Some(Utc::now()),
                message: format!("[mock] {} starting up", self.name),
            }),
            Ok(LogChunk {
                stream: LogStreamKind::Stdout,
                time: Some(Utc::now()),
                message: format!("[mock] {} ready", self.name),
            }),
        ];

        if follow {
            stream::iter(canned).chain(stream::pending()).boxed()
        } else {
            stream::iter(canned).boxed()
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[async_trait]
impl super::Podman for PodmanMock {
    type PImage<'a>
        = PodmanMockImage
    where
        Self: 'a;
    type PContainer = PodmanMockContainer;

    async fn image<'a>(
        &'a self,
        reference: &str,
        _: super::PodmanPullBehaviour,
    ) -> anyhow::Result<Option<Self::PImage<'a>>> {
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(Some(PodmanMockImage {
            reference: reference.to_owned(),
        }))
    }

    async fn prune_images(&mut self) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
}

impl PodmanMock {
    pub const fn new() -> Self {
        Self
    }
}

impl super::PodmanImageInfo for PodmanMockImage {
    fn reference(&self) -> &str {
        &self.reference
    }

    fn digest(&self) -> &str {
        "unknown"
    }
}

#[async_trait]
impl super::PodmanImage for PodmanMockImage {
    type PContainer = PodmanMockContainer;

    async fn create_container(
        &self,
        name: &str,
        _: Option<ContainerConfigV1>,
    ) -> anyhow::Result<Self::PContainer> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(PodmanMockContainer {
            name: name.to_owned(),
            reference: self.reference.clone(),
            state: super::PodmanContainerState::Stopped,
        })
    }
}

impl super::PodmanImageInfo for PodmanMockContainer {
    fn reference(&self) -> &str {
        &self.reference
    }

    fn digest(&self) -> &str {
        "unknown"
    }
}

#[async_trait]
impl super::PodmanContainer for PodmanMockContainer {
    async fn start(&mut self) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(200)).await;
        self.state = super::PodmanContainerState::Running;
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.state = super::PodmanContainerState::Stopped;
        Ok(())
    }

    async fn destroy(self) -> anyhow::Result<()> {
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    async fn state(&self) -> anyhow::Result<super::PodmanContainerState> {
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(self.state)
    }

    async fn wait_for_state_change(
        &self,
        current: super::PodmanContainerState,
    ) -> anyhow::Result<()> {
        let wait_duration = match current {
            super::PodmanContainerState::Stopped | super::PodmanContainerState::Ambiguous => {
                Duration::from_millis(200)
            }
            super::PodmanContainerState::Running => Duration::from_secs(2),
        };
        tokio::time::sleep(wait_duration).await;
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
    type LogHandle = PodmanMockLogHandle;
    fn log_handle(&self) -> Self::LogHandle {
        PodmanMockLogHandle {
            name: self.name.clone(),
        }
    }
}
