use std::{sync::Arc, time::Duration};

use crate::podman::{PodmanContainer, PodmanContainerState, PodmanImage, PodmanImageInfo};

/// Struct to manage lifecycle of an application.
/// Create it from an existing container by calling PodmanContainer::into,
/// then it will try to keep it alive.
///
/// When dropped, leaves the container behind in whichever state it is in!
#[derive(Debug)]
pub struct Application {
    image_reference: String,
    image_digest: String,
    lifecycle_loop: tokio::task::JoinHandle<()>,
    delete_notifier: Arc<tokio::sync::Notify>,
}

impl Application {
    pub fn wrap(container: impl PodmanContainer) -> Self {
        let delete_notifier = Arc::new(tokio::sync::Notify::const_new());
        let event_recv = LogEventReceiver {
            app_name: container.name().to_owned(),
        };
        Application {
            image_reference: container.reference().to_owned(),
            image_digest: container.digest().to_owned(),
            lifecycle_loop: tokio::spawn(run_lifecycle_loop(
                container,
                event_recv,
                delete_notifier.clone(),
            )),
            delete_notifier,
        }
    }

    pub async fn launch_from_image(image: &impl PodmanImage, name: &str) -> anyhow::Result<Self> {
        let container = image.create_container(name, Vec::new()).await?;
        Ok(Self::wrap(container))
    }

    pub async fn remove(mut self) -> anyhow::Result<()> {
        self.delete_notifier.notify_one();
        (&mut self.lifecycle_loop).await?;
        Ok(())
    }
}

impl PodmanImageInfo for Application {
    fn reference(&self) -> &str {
        &self.image_reference
    }

    fn digest(&self) -> &str {
        &self.image_digest
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.lifecycle_loop.abort();
    }
}

/// Try to keep the container alive to best of ability
/// and output some logs otherwise
async fn run_lifecycle_loop(
    mut container: impl PodmanContainer,
    event_recv: impl EventReceiver,
    delete_notifier: Arc<tokio::sync::Notify>,
) {
    loop {
        let mut failure_counter = 0u32;
        let mut old_state = None;

        let error = loop {
            let state = match container.state().await {
                Ok(s) => s,
                Err(e) => break e,
            };
            let state_changed = old_state.is_some_and(|s| s != state);

            if state_changed {
                event_recv.send(LifecycleEvent::StateChange(old_state, state));
            }

            if failure_counter == 10 {
                event_recv.send(LifecycleEvent::FailureThresholdReached(failure_counter));
            }

            let mut timeout = match state {
                PodmanContainerState::Stopped => {
                    // Do not count the initial start as a failure
                    if old_state.is_some() {
                        failure_counter += 1
                    }

                    if let Err(e) = container.start().await {
                        break e;
                    }

                    event_recv.send(LifecycleEvent::AttemptingStart);
                    Duration::from_secs(10)
                }
                PodmanContainerState::Ambiguous => {
                    failure_counter += 1;
                    Duration::from_secs(10)
                }
                PodmanContainerState::Running => {
                    // Reset the failure counter after 10 mins
                    if state_changed {
                        Duration::from_mins(10)
                    } else {
                        failure_counter = 0;
                        Duration::MAX
                    }
                }
            };

            // Speed up unit tests a bit
            if cfg!(test) {
                timeout = Duration::from_millis(100);
            }

            old_state = Some(state);

            tokio::select! {
                _ = tokio::time::sleep(timeout), if timeout < Duration::MAX => {},
                _ = container.wait_for_state_change(state) => {},
                _ = delete_notifier.notified() => {
                    container.destroy().await.unwrap();
                    return
                }
            }
        };

        event_recv.send(LifecycleEvent::FatalError(error));
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

trait EventReceiver {
    fn send(&self, event: LifecycleEvent);
}

#[derive(Debug)]
enum LifecycleEvent {
    StateChange(Option<PodmanContainerState>, PodmanContainerState),
    FailureThresholdReached(u32),
    AttemptingStart,
    FatalError(anyhow::Error),
}

struct LogEventReceiver {
    app_name: String,
}

impl EventReceiver for LogEventReceiver {
    fn send(&self, event: LifecycleEvent) {
        match event {
            LifecycleEvent::StateChange(None, state) => {
                log::info!("Application {} has state {:?}", self.app_name, state)
            }
            LifecycleEvent::StateChange(Some(old_state), new_state) => log::info!(
                "Application {} changed state: {:?} -> {:?}",
                self.app_name,
                old_state,
                new_state
            ),
            LifecycleEvent::FailureThresholdReached(failures) => log::warn!(
                "Application {} seems to have trouble starting (encountered {} failures)",
                self.app_name,
                failures
            ),
            LifecycleEvent::AttemptingStart => log::debug!("Trying to start {}", self.app_name),
            LifecycleEvent::FatalError(e) => log::error!("{:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;

    use super::*;
    use crate::podman::PodmanContainerState::{Ambiguous, Running, Stopped};

    struct ChannelEventReceiver {
        tx: tokio::sync::mpsc::UnboundedSender<LifecycleEvent>,
    }

    impl EventReceiver for ChannelEventReceiver {
        fn send(&self, event: LifecycleEvent) {
            self.tx.send(event).unwrap()
        }
    }

    struct WorkingMockContainer(Arc<Mutex<PodmanContainerState>>);

    #[async_trait]
    impl PodmanContainer for WorkingMockContainer {
        async fn start(&mut self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = PodmanContainerState::Running;
            Ok(())
        }
        async fn stop(&mut self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = PodmanContainerState::Stopped;
            Ok(())
        }
        async fn destroy(self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn state(&self) -> anyhow::Result<PodmanContainerState> {
            Ok(*self.0.lock().unwrap())
        }
        async fn wait_for_state_change(&self, current: PodmanContainerState) -> anyhow::Result<()> {
            if current == *self.0.lock().unwrap() {
                tokio::time::sleep(Duration::MAX).await;
            }
            Ok(())
        }
        fn name(&self) -> &str {
            "testname"
        }
    }

    impl PodmanImageInfo for WorkingMockContainer {
        fn reference(&self) -> &str {
            "docker.io/library/alpine:latest"
        }
        fn digest(&self) -> &str {
            "unknown"
        }
    }

    #[tokio::test]
    async fn test_application_working() {
        let container_state = Arc::new(Mutex::new(PodmanContainerState::Stopped));
        let container = WorkingMockContainer(container_state.clone());

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
        ));

        assert!(matches!(
            rx.recv().await.unwrap(),
            LifecycleEvent::AttemptingStart
        ));
        assert!(matches!(
            rx.recv().await.unwrap(),
            LifecycleEvent::StateChange(Some(Stopped), Running)
        ));
        assert_eq!(
            *container_state.lock().unwrap(),
            PodmanContainerState::Running
        );

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
    }

    struct NotStartingMockContainer;

    #[async_trait]
    impl PodmanContainer for NotStartingMockContainer {
        async fn start(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn stop(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn destroy(self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn state(&self) -> anyhow::Result<PodmanContainerState> {
            Ok(PodmanContainerState::Stopped)
        }
        async fn wait_for_state_change(&self, current: PodmanContainerState) -> anyhow::Result<()> {
            if current == PodmanContainerState::Stopped {
                tokio::time::sleep(Duration::MAX).await;
            }
            Ok(())
        }
        fn name(&self) -> &str {
            "testname"
        }
    }

    impl PodmanImageInfo for NotStartingMockContainer {
        fn reference(&self) -> &str {
            "docker.io/library/alpine:latest"
        }
        fn digest(&self) -> &str {
            "unknown"
        }
    }

    #[tokio::test]
    async fn test_application_not_starting() {
        let container = NotStartingMockContainer;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
        ));

        // Make sure after some amount of failures the system throws an error
        for i in 0.. {
            match rx.recv().await.unwrap() {
                LifecycleEvent::AttemptingStart => assert!(i < 100),
                LifecycleEvent::FailureThresholdReached(failures) => {
                    assert_eq!(i - 1, failures);
                    break;
                }
                _ => panic!(),
            }
        }

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
    }

    struct AmbiguousMockContainer(Arc<Mutex<PodmanContainerState>>);

    #[async_trait]
    impl PodmanContainer for AmbiguousMockContainer {
        async fn start(&mut self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = PodmanContainerState::Ambiguous;
            Ok(())
        }
        async fn stop(&mut self) -> anyhow::Result<()> {
            *self.0.lock().unwrap() = PodmanContainerState::Stopped;
            Ok(())
        }
        async fn destroy(self) -> anyhow::Result<()> {
            Ok(())
        }
        async fn state(&self) -> anyhow::Result<PodmanContainerState> {
            Ok(*self.0.lock().unwrap())
        }
        async fn wait_for_state_change(&self, current: PodmanContainerState) -> anyhow::Result<()> {
            if current == *self.0.lock().unwrap() {
                tokio::time::sleep(Duration::MAX).await;
            }
            Ok(())
        }
        fn name(&self) -> &str {
            "testname"
        }
    }

    impl PodmanImageInfo for AmbiguousMockContainer {
        fn reference(&self) -> &str {
            "docker.io/library/alpine:latest"
        }
        fn digest(&self) -> &str {
            "unknown"
        }
    }

    #[tokio::test]
    async fn test_application_ambiguous() {
        let container_state = Arc::new(Mutex::new(PodmanContainerState::Stopped));
        let container = AmbiguousMockContainer(container_state.clone());

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
        ));

        assert!(matches!(
            rx.recv().await.unwrap(),
            LifecycleEvent::AttemptingStart
        ));
        assert!(matches!(
            rx.recv().await.unwrap(),
            LifecycleEvent::StateChange(Some(Stopped), Ambiguous)
        ));
        assert!(matches!(
            rx.recv().await.unwrap(),
            LifecycleEvent::FailureThresholdReached(_)
        ));

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
    }
}
