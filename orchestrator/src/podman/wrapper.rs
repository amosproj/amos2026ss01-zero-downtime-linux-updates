// Control Podman
// This assumes full ownership of the Podman instance

use std::path::Path;

use async_trait::async_trait;
use futures_util::StreamExt;
use podman_api::{
    models::{ContainerStatus, LibpodImagesPullReport},
    opts::{
        ContainerCreateOpts, ContainerDeleteOpts, ContainerListOpts, ContainerStopOpts,
        ContainerWaitOpts, ImagePruneOpts, PullOpts,
    },
};
use tracing::debug;

use super::{PodmanContainerState, PodmanPullBehaviour};

pub struct PodmanWrapper {
    podman: podman_api::Podman,
}

#[async_trait]
impl super::Podman for PodmanWrapper {
    type PImage<'a>
        = PodmanWrapperImage<'a>
    where
        Self: 'a;
    type PContainer = PodmanWrapperContainer;

    async fn image<'a>(
        &'a self,
        reference: &str,
        behaviour: PodmanPullBehaviour,
    ) -> anyhow::Result<Option<Self::PImage<'a>>> {
        let policy = match behaviour {
            PodmanPullBehaviour::AlwaysPull => podman_api::opts::PullPolicy::Always,
            PodmanPullBehaviour::PullIfMissingOrNewer => podman_api::opts::PullPolicy::Newer,
            PodmanPullBehaviour::PullIfMissing => podman_api::opts::PullPolicy::Missing,
            PodmanPullBehaviour::NeverPull => podman_api::opts::PullPolicy::Never,
        };

        let pi = self.podman.images();
        let mut events = pi.pull(
            &PullOpts::builder()
                .policy(policy)
                .quiet(true)
                .reference(reference)
                .build(),
        );

        // According to the source, we expect a single response packet with all the image IDs
        // https://github.com/containers/podman/blob/62111c7e9d2c20c8bad81fe18359685c3ba6aeb2/pkg/api/handlers/libpod/images_pull.go#L131
        let images = match events.next().await {
            Some(Ok(LibpodImagesPullReport {
                images: Some(imgs), ..
            })) => imgs,
            Some(Ok(LibpodImagesPullReport { error: Some(e), .. }))
                if behaviour == PodmanPullBehaviour::NeverPull
                    && e.ends_with("image not known") =>
            {
                return Ok(None);
            }
            Some(Ok(LibpodImagesPullReport { error: Some(e), .. })) => anyhow::bail!(e),
            Some(Err(e)) => return Err(e.into()),
            _ => anyhow::bail!("Invalid response"),
        };
        if events.next().await.is_some() {
            anyhow::bail!("Too many response entries");
        }
        let image_id = match images.as_slice() {
            [id] => id.to_owned(),
            _ => anyhow::bail!("Pulled {} images, which is weird", images.len()),
        };

        // e.g. "alpine" -> "docker.io/library/alpine:latest"
        let image_data = pi.get(&image_id).inspect().await?;
        let reference = image_data
            .names_history
            .and_then(|mut h| h.pop())
            .unwrap_or(reference.to_owned());
        let digest = image_data
            .digest
            .ok_or(anyhow::anyhow!("Could not read image digest"))?;

        Ok(Some(PodmanWrapperImage {
            podman: self,
            id: image_id,
            reference,
            digest,
        }))
    }

    async fn prune_images(&mut self) -> anyhow::Result<()> {
        self.podman
            .images()
            .prune(&ImagePruneOpts::builder().all(true).build())
            .await?;
        Ok(())
    }
}

impl PodmanWrapper {
    /// Assumes full ownership, do not call multiple times on the same socket!!
    /// This spits out a connected instance as well as references to all the
    /// pre-existing containers
    pub async fn connect(
        socket_path: &Path,
    ) -> anyhow::Result<(Self, Vec<PodmanWrapperContainer>)> {
        let podman = podman_api::Podman::unix(socket_path);

        let status = podman.ping().await?;
        debug!("Connected to Podman API {}", status.api_version);

        let mut instance = Self { podman };
        let containers = instance.list_containers().await?;

        Ok((instance, containers))
    }

    #[cfg(test)]
    async fn list_images<'a>(&'a mut self) -> anyhow::Result<Vec<PodmanWrapperImage<'a>>> {
        let images = self
            .podman
            .images()
            .list(&podman_api::opts::ImageListOpts::default())
            .await?
            .into_iter()
            .filter_map(|i| {
                Some(PodmanWrapperImage {
                    podman: self,
                    id: i.id?,
                    reference: i.names?.pop()?,
                    digest: i.digest?,
                })
            })
            .collect();
        Ok(images)
    }

    async fn list_containers(&mut self) -> anyhow::Result<Vec<PodmanWrapperContainer>> {
        let container_futs = self
            .podman
            .containers()
            .list(&ContainerListOpts::builder().all(true).build())
            .await?
            .into_iter()
            .map(async |c: podman_api::models::ListContainer| {
                let image = self.podman.images().get(c.image_id?).inspect().await.ok()?;
                Some(PodmanWrapperContainer {
                    container: self.podman.containers().get(c.id.as_deref()?),
                    id: c.id?,
                    name: c.names?.pop()?,
                    image_ref: image.names_history?.pop()?,
                    image_digest: image.digest?,
                })
            });

        Ok(futures_util::future::join_all(container_futs)
            .await
            .into_iter()
            .flatten()
            .collect())
    }
}

pub struct PodmanWrapperImage<'a> {
    podman: &'a PodmanWrapper,
    id: String,
    reference: String,
    digest: String,
}

impl<'a> super::PodmanImageInfo for PodmanWrapperImage<'a> {
    fn reference(&self) -> &str {
        &self.reference
    }

    fn digest(&self) -> &str {
        &self.digest
    }
}

#[async_trait]
impl<'a> super::PodmanImage for PodmanWrapperImage<'a> {
    type PContainer = PodmanWrapperContainer;

    async fn create_container(
        &self,
        name: &str,
        environment: impl IntoIterator<Item = (&str, &str)> + Send,
    ) -> anyhow::Result<Self::PContainer> {
        let pc = self.podman.podman.containers();
        let output = pc
            .create(
                &ContainerCreateOpts::builder()
                    .name(name)
                    .image(&self.id)
                    .env(environment)
                    .build(),
            )
            .await?;

        Ok(PodmanWrapperContainer {
            container: pc.get(&output.id),
            id: output.id,
            name: name.to_owned(),
            image_ref: self.reference.clone(),
            image_digest: self.digest.clone(),
        })
    }
}

pub struct PodmanWrapperContainer {
    container: podman_api::api::Container,
    id: String,
    name: String,
    image_ref: String,
    image_digest: String,
}

impl super::PodmanImageInfo for PodmanWrapperContainer {
    fn reference(&self) -> &str {
        &self.image_ref
    }

    fn digest(&self) -> &str {
        &self.image_digest
    }
}

#[async_trait]
impl super::PodmanContainer for PodmanWrapperContainer {
    async fn start(&mut self) -> anyhow::Result<()> {
        if self.state().await? == PodmanContainerState::Stopped {
            self.container.start(None).await?;
        }
        Ok(())
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        if self.state().await? == PodmanContainerState::Running {
            self.container
                .stop(
                    &ContainerStopOpts::builder()
                        .ignore(true)
                        .timeout(10)
                        .build(),
                )
                .await?;
        }
        Ok(())
    }

    async fn destroy(self) -> anyhow::Result<()> {
        self.container
            .delete(&ContainerDeleteOpts::default())
            .await?;
        Ok(())
    }

    async fn state(&self) -> anyhow::Result<PodmanContainerState> {
        let status = self.container.inspect().await?.state.and_then(|s| s.status);
        Ok(match status {
            Some(s) => PodmanContainerState::from(s.as_ref()),
            None => PodmanContainerState::Ambiguous,
        })
    }

    async fn wait_for_state_change(
        &self,
        current: super::PodmanContainerState,
    ) -> anyhow::Result<()> {
        use podman_api::models::ContainerStatus::*;
        let all_conditions: [ContainerStatus; _] = [
            Configured, Created, Dead, Exited, Paused, Removing, Restarting, Running,
        ];
        self.container
            .wait(
                &ContainerWaitOpts::builder()
                    .conditions(
                        // Wait for another condition as currently known
                        all_conditions
                            .into_iter()
                            .filter(|c| current != c.as_ref().into()),
                    )
                    .build(),
            )
            .await?;
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl From<&str> for PodmanContainerState {
    fn from(value: &str) -> Self {
        match value {
            "created" | "initialized" | "stopped" | "exited" | "paused" => Self::Stopped,
            "running" => Self::Running,
            _ => Self::Ambiguous,
        }
    }
}

impl From<ContainerStatus> for PodmanContainerState {
    fn from(value: ContainerStatus) -> Self {
        value.as_ref().into()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::podman::{
        Podman, PodmanContainer, PodmanContainerState, PodmanImage, PodmanImageInfo,
        PodmanPullBehaviour::PullIfMissing,
    };

    const PODMAN_SOCK: &str = "/run/podman/podman.sock";

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    #[serial_test::serial]
    async fn test_podman_image_prune() {
        let (mut p, _) = super::PodmanWrapper::connect(Path::new(PODMAN_SOCK))
            .await
            .unwrap();
        p.prune_images().await.unwrap();
        let len_initial = p.list_images().await.unwrap().len();

        p.image(
            "hello-world:latest",
            crate::podman::PodmanPullBehaviour::PullIfMissing,
        )
        .await
        .unwrap();

        assert_eq!(p.list_images().await.unwrap().len(), len_initial + 1);

        p.prune_images().await.unwrap();

        assert_eq!(p.list_images().await.unwrap().len(), len_initial);
    }

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    #[serial_test::serial]
    async fn test_podman_image_pull() {
        let (p, _) = super::PodmanWrapper::connect(Path::new(PODMAN_SOCK))
            .await
            .unwrap();

        let img = p
            .image("alpine", crate::podman::PodmanPullBehaviour::PullIfMissing)
            .await
            .unwrap();
        assert_eq!(img.is_some(), true);

        let img = p
            .image(
                "asdfhakdhjsfklashdf",
                crate::podman::PodmanPullBehaviour::NeverPull,
            )
            .await
            .unwrap();
        assert_eq!(img.is_some(), false);
    }

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    #[serial_test::serial]
    async fn test_podman_create_and_destroy() {
        let (mut p, _) = super::PodmanWrapper::connect(Path::new(PODMAN_SOCK))
            .await
            .unwrap();
        let lc =
            async |p: &mut super::PodmanWrapper| p.list_containers().await.unwrap().into_iter();

        assert!(
            lc(&mut p).await.all(|c| c.name() != "test-container"),
            "Container already exists"
        );

        let img = p
            .image("alpine", crate::podman::PodmanPullBehaviour::PullIfMissing)
            .await
            .unwrap()
            .unwrap();

        let container = img
            .create_container("test-container", vec![])
            .await
            .unwrap();

        assert_eq!(container.name(), "test-container");
        assert_eq!(container.reference(), "docker.io/library/alpine:latest");
        assert_eq!(
            container.state().await.unwrap(),
            PodmanContainerState::Stopped
        );

        assert!(
            lc(&mut p).await.any(|c| c.name() == "test-container"),
            "Container not created"
        );

        let list_container = p
            .list_containers()
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.name() == "test-container")
            .unwrap();

        assert_eq!(list_container.name(), "test-container");
        assert_eq!(
            list_container.reference(),
            "docker.io/library/alpine:latest"
        );
        assert_eq!(
            list_container.state().await.unwrap(),
            PodmanContainerState::Stopped
        );

        container.destroy().await.unwrap();

        assert!(
            lc(&mut p).await.all(|c| c.name() != "test-container"),
            "Container not destroyed"
        );
    }

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    #[serial_test::serial]
    async fn test_podman_wait_for_state() {
        let (p, _) = super::PodmanWrapper::connect(Path::new(PODMAN_SOCK))
            .await
            .unwrap();

        let image = p
            .image("docker.io/valkey/valkey", PullIfMissing)
            .await
            .unwrap()
            .unwrap();
        let mut container = image.create_container("test", vec![]).await.unwrap();

        assert_eq!(
            container.state().await.unwrap(),
            PodmanContainerState::Stopped
        );

        // Should immediately return
        container
            .wait_for_state_change(PodmanContainerState::Running)
            .await
            .unwrap();

        container.start().await.unwrap();
        container
            .wait_for_state_change(PodmanContainerState::Stopped)
            .await
            .unwrap();

        assert_eq!(
            container.state().await.unwrap(),
            PodmanContainerState::Running
        );
        container.stop().await.unwrap();
        container.destroy().await.unwrap();
    }
}
