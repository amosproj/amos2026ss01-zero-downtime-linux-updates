//! Logic to repeatedly check for application updates and apply them

use std::iter::Peekable;
use std::sync::Arc;
use std::time::Duration;

use amos_common::entities::ApplicationConfig;
use tracing::warn;

use crate::api_client::ApiClient;
use crate::application::Application;
use crate::podman::PodmanPullBehaviour::PullIfMissing;
use crate::podman::log_registry::AppLogRegistry;
use crate::podman::{Podman, PodmanImage, PodmanImageInfo};

/// Repeatedly check for application updates and apply them
pub async fn run_apps_main_loop(
    mut apps: Vec<Application>,
    mut podman: impl Podman,
    api_client: Arc<ApiClient>,
    poll_interval: Duration,
) -> ! {
    let mut update_interval = tokio::time::interval(poll_interval);
    // Prevent bursting should an update cycle take longer than expected
    update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        update_interval.tick().await;

        if let Err(e) = try_update(&mut apps, &mut podman, &api_client).await {
            warn!("Failed to update applications: {:?}", e);
        }
    }
}

#[allow(dead_code)]
pub fn resolve_application_ids<C: PodmanImageInfo>(
    containers: Vec<C>,
    target_app_configs: &[ApplicationConfig::Model],
) -> (Vec<(C, i32)>, Vec<C>) {
    let mut matched = Vec::new();
    let mut unmatched = Vec::new();

    for container in containers {
        let app_ref = container
            .reference()
            .split(':')
            .next()
            .unwrap_or(container.reference());
        let found = target_app_configs
            .iter()
            .find(|cfg| cfg.image.split(':').next().unwrap_or(&cfg.image) == app_ref);

        match found {
            Some(cfg) => matched.push((container, cfg.id)),
            None => unmatched.push(container),
        }
    }

    (matched, unmatched)
}

async fn try_update(
    apps: &mut Vec<Application>,
    podman: &mut impl Podman,
    api_client: &ApiClient,
) -> anyhow::Result<()> {
    // First, pull target config and possibly new images
    let target_app_configs = api_client.get_target_application_configs().await?;
    let mut target = futures_util::future::join_all(
        target_app_configs
            .iter()
            .map(|a| TargetApp::from_config(a, podman)),
    )
    .await
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    target.sort_by(order_reference);

    apps.sort_by(order_reference);

    for action in ReconcileIterator::new(apps, target) {
        match action {
            ReconcileAction::Create { image } => {
                let app =
                    Application::launch_from_image(&image.image, image.name, image.environment)
                        .await?;
                apps.push(app);
            }
            ReconcileAction::Update {
                application_index,
                target_image,
            } => {
                apps.swap_remove(application_index).remove().await?;
                let app = Application::launch_from_image(
                    &target_image.image,
                    target_image.name,
                    target_image.environment,
                )
                .await?;
                apps.push(app);
            }
            ReconcileAction::Remove { application_index } => {
                apps.swap_remove(application_index).remove().await?;
            }
        }
    }

    for app in target_app_configs {
        api_client
            .report_current_application_assignment(app.id)
            .await?;
    }

    podman.prune_images().await?;

    Ok(())
}

struct TargetApp<'a, P: PodmanImage> {
    image: P,
    name: &'a str,
    environment: Vec<(&'a str, &'a str)>,
}

impl<'a, P: PodmanImage> TargetApp<'a, P> {
    async fn from_config(
        cfg: &'a ApplicationConfig::Model,
        podman: &'a impl Podman<PImage<'a> = P>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            image: podman
                .image(&cfg.image, PullIfMissing)
                .await?
                .ok_or(anyhow::anyhow!("Could not pull image"))?,
            name: &cfg.image,
            environment: vec![],
        })
    }
}

impl<'a, P: PodmanImage> PodmanImageInfo for TargetApp<'a, P> {
    fn reference(&self) -> &str {
        self.image.reference()
    }

    fn digest(&self) -> &str {
        self.image.digest()
    }
}

/// Iterator to generate actions for merging the target application state
/// into the current one. Returns the actions in reverse order of the current
/// applications, so the Vec can be modified in a "for"-loop.
struct ReconcileIterator<PImg: PodmanImageInfo> {
    current: Peekable<std::vec::IntoIter<(String, String, usize)>>,
    target: Peekable<std::vec::IntoIter<(String, PImg)>>,
}

impl<PImg: PodmanImageInfo> ReconcileIterator<PImg> {
    /// Both apps and target_imgs have to be sorted by image reference
    fn new(
        apps: &[impl PodmanImageInfo],
        target_imgs: impl IntoIterator<IntoIter = impl DoubleEndedIterator<Item = PImg>>,
    ) -> Self {
        let current: Vec<_> = apps
            .iter()
            .enumerate()
            .rev()
            .map(|(i, app)| {
                // Remove any tags from image reference
                let reference = app.reference().split(':').next().unwrap().to_owned();
                (reference, app.digest().to_owned(), i)
            })
            .collect();

        let target: Vec<_> = target_imgs
            .into_iter()
            .rev()
            .map(|img| {
                // Remove any tags from image reference
                let reference = img.reference().split(':').next().unwrap().to_owned();
                (reference, img)
            })
            .collect();

        debug_assert!(current.is_sorted_by(|a, b| a.0 > b.0));
        debug_assert!(target.is_sorted_by(|a, b| a.0 > b.0));

        Self {
            current: current.into_iter().peekable(),
            target: target.into_iter().peekable(),
        }
    }
}

impl<PI: PodmanImageInfo> Iterator for ReconcileIterator<PI> {
    type Item = ReconcileAction<PI>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Early exits for empty iterators
            let (curr, target) = match (self.current.peek(), self.target.peek()) {
                (None, None) => return None,
                (None, _) => {
                    return Some(ReconcileAction::Create {
                        image: self.target.next()?.1,
                    });
                }
                (_, None) => {
                    return Some(ReconcileAction::Remove {
                        application_index: self.current.next()?.2,
                    });
                }
                (Some(c), Some(t)) => (c, t),
            };

            match (&*curr.0, &*target.0) {
                (a, b) if a < b => {
                    return Some(ReconcileAction::Create {
                        image: self.target.next()?.1,
                    });
                }
                (a, b) if a > b => {
                    return Some(ReconcileAction::Remove {
                        application_index: self.current.next()?.2,
                    });
                }
                _ => {
                    let curr = self.current.next()?;
                    let target = self.target.next()?;

                    // Digests are different if we pulled a different image earlier
                    if curr.1 != target.1.digest() {
                        return Some(ReconcileAction::Update {
                            application_index: curr.2,
                            target_image: target.1,
                        });
                    }
                }
            }
        }
    }
}

enum ReconcileAction<PI: PodmanImageInfo> {
    Create {
        image: PI,
    },
    Update {
        application_index: usize,
        target_image: PI,
    },
    Remove {
        application_index: usize,
    },
}

fn order_reference<A: PodmanImageInfo, B: PodmanImageInfo>(a: &A, b: &B) -> std::cmp::Ordering {
    a.reference().cmp(b.reference())
}

#[cfg(test)]
mod tests {
    use super::{ReconcileAction::*, *};

    #[derive(Clone, Copy)]
    struct MockApplication<'a> {
        reference: &'a str,
        digest: &'a str,
    }

    impl<'a> MockApplication<'a> {
        fn new(reference: &'a str, digest: &'a str) -> Self {
            Self { reference, digest }
        }
    }

    impl<'a> PodmanImageInfo for MockApplication<'a> {
        fn reference(&self) -> &str {
            &self.reference
        }

        fn digest(&self) -> &str {
            &self.digest
        }
    }

    #[test]
    fn test_reconcile() {
        let alpine_1 = MockApplication::new("docker.io/alpine:1.0", "alpine_1");
        let alpine_2 = MockApplication::new("docker.io/alpine:2.0", "alpine_2");
        let mongodb = MockApplication::new("docker.io/mongodb:5.0", "mongodb");
        let postgres = MockApplication::new("docker.io/postgres:asöldfjasd", "postgres");

        let apps = [alpine_1, postgres];
        let target = [alpine_2, mongodb];

        let mut iter = ReconcileIterator::new(&apps, target);

        assert!(matches!(
            iter.next(),
            Some(Remove {
                application_index: 1
            })
        ));
        assert!(matches!(
            iter.next(),
            Some(Create {
                image: MockApplication {
                    digest: "mongodb",
                    ..
                }
            })
        ));
        assert!(matches!(
            iter.next(),
            Some(Update {
                application_index: 0,
                target_image: MockApplication {
                    digest: "alpine_2",
                    ..
                }
            })
        ));
        assert!(iter.next().is_none())
    }

    #[test]
    fn resolve_application_ids_matches_by_reference() {
        let configs = vec![ApplicationConfig::Model {
            id: 1,
            application_id: 1,
            config: Some("testconfig".into()),
            image: "docker.io/alpine:1.0".into(),
            comment: Some("testcomment".into()),
            deleted_at: None,
            superseded_by: None,
        }];
        let containers = vec![MockApplication::new("docker.io/alpine:1.0", "digest1")];
        let (matched, unmatched) = resolve_application_ids(containers, &configs);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].1, 1);
        assert!(unmatched.is_empty());
    }
}
