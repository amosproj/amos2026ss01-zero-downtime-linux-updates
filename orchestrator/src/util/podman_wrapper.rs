// TODO: Remove this once this is integrated
#![allow(dead_code)]

// Control Podman
// This assumes full ownership of the Podman instance

use std::sync::atomic::{AtomicBool, Ordering};

use futures_util::StreamExt;
use podman_api::{
    models::LibpodImagesPullReport,
    opts::{
        ContainerCreateOpts, ContainerDeleteOpts, ContainerListOpts, ContainerStopOpts,
        ImagePruneOpts, PullOpts,
    },
};

type PodmanErr<R> = Result<R, Box<dyn std::error::Error>>;

const PODMAN_SOCKET_PATH: &str = "/run/podman/podman.sock";

// Ensure only a single instance of this ever exists
static INSTANCE_TAKEN: AtomicBool = AtomicBool::new(false);

pub struct Podman {
    podman: podman_api::Podman,
}

impl Podman {
    pub async fn take() -> PodmanErr<(Self, Vec<PodmanContainer>)> {
        if INSTANCE_TAKEN.swap(true, Ordering::Relaxed) {
            return Err("Cannot create multiple Podman wrapper instances".into());
        }

        let podman = podman_api::Podman::unix(PODMAN_SOCKET_PATH);

        let status = podman.ping().await?;
        log::debug!("Connected to Podman API {}", status.api_version);

        let mut instance = Self { podman };
        let containers = instance.list_containers().await?;

        Ok((instance, containers))
    }

    #[cfg(test)]
    async fn list_images<'a>(&'a mut self) -> PodmanErr<Vec<PodmanImage<'a>>> {
        let images = self
            .podman
            .images()
            .list(&podman_api::opts::ImageListOpts::default())
            .await?
            .into_iter()
            .filter_map(|i| {
                Some(PodmanImage {
                    podman: self,
                    id: i.id?,
                    reference: i.names?.pop()?,
                })
            })
            .collect();
        Ok(images)
    }

    async fn list_containers(&mut self) -> PodmanErr<Vec<PodmanContainer>> {
        let containers = self
            .podman
            .containers()
            .list(&ContainerListOpts::builder().all(true).build())
            .await?
            .into_iter()
            .filter_map(|c| {
                Some(PodmanContainer {
                    container: self.podman.containers().get(c.id.as_deref()?),
                    id: c.id?,
                    name: c.names?.pop()?,
                    image_ref: c.image?,
                })
            })
            .collect();
        Ok(containers)
    }

    pub async fn image<'a>(
        &'a self,
        reference: String,
        behaviour: PodmanPullBehaviour,
    ) -> PodmanErr<Option<PodmanImage<'a>>> {
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
                .reference(reference.clone())
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
            Some(Ok(LibpodImagesPullReport { error: Some(e), .. })) => return Err(e.into()),
            Some(Err(e)) => return Err(e.into()),
            _ => return Err("Invalid response".into()),
        };
        if matches!(events.next().await, Some(_)) {
            return Err("Too many response entries".into());
        }
        let image_id = match images.as_slice() {
            [id] => id.to_owned(),
            _ => return Err(format!("Pulled {} images, which is weird", images.len()).into()),
        };

        // e.g. "alpine" -> "docker.io/library/alpine:latest"
        let full_reference = pi
            .get(&image_id)
            .inspect()
            .await?
            .names_history
            .and_then(|mut v| v.pop())
            .unwrap_or(reference);

        Ok(Some(PodmanImage {
            podman: self,
            id: image_id,
            reference: full_reference,
        }))
    }

    pub async fn prune_images(&mut self) -> PodmanErr<()> {
        self.podman
            .images()
            .prune(&ImagePruneOpts::builder().all(true).build())
            .await?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodmanPullBehaviour {
    AlwaysPull,
    PullIfMissingOrNewer,
    PullIfMissing,
    NeverPull,
}

pub struct PodmanImage<'a> {
    podman: &'a Podman,
    id: String,
    reference: String,
}

impl<'a> PodmanImage<'a> {
    pub async fn create_container(
        &self,
        name: String,
        environment: impl IntoIterator<Item = (&str, &str)>,
    ) -> PodmanErr<PodmanContainer> {
        let pc = self.podman.podman.containers();
        let output = pc
            .create(
                &ContainerCreateOpts::builder()
                    .name(&name)
                    .image(&self.id)
                    .env(environment)
                    .build(),
            )
            .await?;

        Ok(PodmanContainer {
            container: pc.get(&output.id),
            id: output.id,
            name,
            image_ref: self.reference.clone(),
        })
    }
}

pub struct PodmanContainer {
    container: podman_api::api::Container,
    id: String,
    name: String,
    image_ref: String,
}

impl PodmanContainer {
    pub async fn start(&mut self) -> PodmanErr<()> {
        if self.state().await? == PodmanContainerState::Stopped {
            self.container.start(None).await?;
        }
        Ok(())
    }

    pub async fn stop(&mut self) -> PodmanErr<()> {
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

    pub async fn destroy(self) -> PodmanErr<()> {
        self.container
            .delete(&ContainerDeleteOpts::default())
            .await?;
        Ok(())
    }

    pub async fn state(&self) -> PodmanErr<PodmanContainerState> {
        let status = self.container.inspect().await?.state.and_then(|s| s.status);
        Ok(match status {
            Some(s) => PodmanContainerState::from(s.as_ref()),
            None => PodmanContainerState::Ambiguous,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn image_ref(&self) -> &str {
        &self.image_ref
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PodmanContainerState {
    Stopped,
    Ambiguous,
    Running,
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

#[cfg(test)]
mod tests {
    use crate::util::podman_wrapper::PodmanContainerState;

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    async fn test_podman_image_prune() {
        let mut p = super::Podman {
            podman: podman_api::Podman::unix(super::PODMAN_SOCKET_PATH),
        };
        let len_initial = p.list_images().await.unwrap().len();

        p.image(
            "alpine:latest".to_owned(),
            super::PodmanPullBehaviour::PullIfMissing,
        )
        .await
        .unwrap();

        assert_eq!(p.list_images().await.unwrap().len(), len_initial + 1);

        p.prune_images().await.unwrap();

        assert_eq!(p.list_images().await.unwrap().len(), len_initial);
    }

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    async fn test_podman_image_pull() {
        let p = super::Podman {
            podman: podman_api::Podman::unix(super::PODMAN_SOCKET_PATH),
        };

        let img = p
            .image(
                "alpine".to_owned(),
                super::PodmanPullBehaviour::PullIfMissing,
            )
            .await
            .unwrap();
        assert_eq!(img.is_some(), true);

        let img = p
            .image(
                "asdfhakdhjsfklashdf".to_owned(),
                super::PodmanPullBehaviour::NeverPull,
            )
            .await
            .unwrap();
        assert_eq!(img.is_some(), false);
    }

    #[tokio::test]
    #[ignore = "has to have access to the Podman socket"]
    async fn test_podman_create_and_destroy() {
        let mut p = super::Podman {
            podman: podman_api::Podman::unix(super::PODMAN_SOCKET_PATH),
        };
        let lc = async |p: &mut super::Podman| p.list_containers().await.unwrap().into_iter();

        assert!(
            lc(&mut p).await.all(|c| c.name() != "test-container"),
            "Container already exists"
        );

        let img = p
            .image(
                "alpine".to_owned(),
                super::PodmanPullBehaviour::PullIfMissing,
            )
            .await
            .unwrap()
            .unwrap();

        let container = img
            .create_container("test-container".to_owned(), vec![])
            .await
            .unwrap();

        assert_eq!(container.name(), "test-container");
        assert_eq!(container.image_ref(), "docker.io/library/alpine:latest");
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
            list_container.image_ref(),
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
}
