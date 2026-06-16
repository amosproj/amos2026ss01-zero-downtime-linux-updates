use std::iter::Peekable;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

use crate::application::Application;
use crate::download_manager::DownloadManager;
use crate::podman::PodmanPullBehaviour::PullIfMissing;
use crate::podman::{Podman, PodmanImageInfo};
use crate::state::AgentState;

pub async fn run_apps_main_loop(
    agent_state: AgentState,
    podman: impl Podman,
    download_manager: Arc<DownloadManager>,
) {
    let mut update_interval = tokio::time::interval(Duration::from_secs(
        agent_state.config.poll_interval_secs as u64,
    ));
    // Prevent bursting should an update cycle take longer than expected
    update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        update_interval.tick().await;

        if let Err(e) = try_update(&agent_state.apps_state, &podman, &download_manager).await {
            log::warn!("Failed to update applications: {:?}", e);
        }
    }
}

async fn try_update(
    apps_state: &Mutex<Vec<Application>>,
    podman: &impl Podman,
    download_manager: &DownloadManager,
) -> anyhow::Result<()> {
    let target_apps = download_manager.get_target_application_configs().await?;

    // Pull new images
    let mut target_images = futures_util::future::join_all(
        target_apps
            .iter()
            .map(|a| podman.image(&a.image, PullIfMissing)),
    )
    .await
    .into_iter()
    .map(|r| r.and_then(|o| o.ok_or(anyhow::anyhow!("Could not pull image"))))
    .collect::<Result<Vec<_>, _>>()?;

    target_images.sort_by(order_reference);

    {
        let mut apps = apps_state.lock().await;

        apps.sort_by(order_reference);

        for action in ReconcileIterator::new(&apps, target_images) {
            match action {
                ReconcileAction::Create { image } => {
                    let app = Application::launch_from_image(&image, image.digest()).await?;
                    apps.push(app);
                }
                ReconcileAction::Update {
                    application_index,
                    target_image,
                } => {
                    apps.swap_remove(application_index).remove().await?;
                    let app = Application::launch_from_image(&target_image, target_image.digest())
                        .await?;
                    apps.push(app);
                }
                ReconcileAction::Remove { application_index } => {
                    apps.swap_remove(application_index).remove().await?;
                }
            }
        }
    }

    for app in target_apps {
        download_manager
            .report_current_application_assignment(app.id)
            .await?;
    }

    Ok(())
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
}
