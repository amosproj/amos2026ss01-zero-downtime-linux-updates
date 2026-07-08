//! Wrap Podman containers, then manage their lifecycle and logs

use std::{sync::Arc, time::Duration};
use tracing::{debug, error, info, warn};

use amos_common::entities::ContainerConfigV1;

use crate::podman::log_registry::AppLogRegistry;
use crate::podman::{
    PodmanContainer, PodmanContainerState, PodmanImage, PodmanImageInfo, PodmanLogHandle,
};
/// Struct to manage lifecycle of an application.
/// Create it from an existing container by calling PodmanContainer::into,
/// then it will try to keep it alive.
///
/// When dropped, leaves the container behind in whichever state it is in!
#[derive(Debug)]
pub struct Application {
    image_reference: String,
    image_digest: String,
    application_id: i32,
    application_config_id: Option<i32>,
    lifecycle_loop: tokio::task::JoinHandle<()>,
    delete_notifier: Arc<tokio::sync::Notify>,
}

impl Application {
    pub fn wrap(container: impl PodmanContainer, registry: &AppLogRegistry) -> Self {
        let delete_notifier = Arc::new(tokio::sync::Notify::const_new());
        let event_recv = LogEventReceiver {
            app_name: container.name().to_owned(),
        };
        let application_id = container.application_id().unwrap_or(0);
        let application_config_id = container.application_config_id();

        Application {
            image_reference: container.reference().to_owned(),
            image_digest: container.digest().to_owned(),
            application_id,
            application_config_id,
            lifecycle_loop: tokio::spawn(run_lifecycle_loop(
                container,
                event_recv,
                delete_notifier.clone(),
                registry.clone(),
                application_id,
            )),
            delete_notifier,
        }
    }

    pub async fn launch_from_image(
        image: &impl PodmanImage,
        name: &str,
        config: Option<ContainerConfigV1>,
        application_id: i32,
        application_config_id: i32,
        registry: &AppLogRegistry,
    ) -> anyhow::Result<Self> {
        let container = image
            .create_container(name, config, application_id, application_config_id)
            .await?;
        Ok(Self::wrap(container, registry))
    }

    pub async fn remove(mut self, registry: &AppLogRegistry) -> anyhow::Result<()> {
        registry.remove(self.application_id);
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

    fn application_config_id(&self) -> Option<i32> {
        self.application_config_id
    }
}

impl Drop for Application {
    fn drop(&mut self) {
        self.lifecycle_loop.abort();
    }
}

/// Try to keep the container alive to best of ability
/// and output some logs otherwise
async fn run_lifecycle_loop<C: PodmanContainer>(
    mut container: C,
    event_recv: impl EventReceiver,
    delete_notifier: Arc<tokio::sync::Notify>,
    log_registry: AppLogRegistry,
    application_id: i32,
) {
    // Taken once the container yields it; reused across re-registrations so
    // a stream that ends (container not running yet, or stopped) can be
    // reopened later without needing a fresh log handle.
    let mut log_handle: Option<C::LogHandle> = None;
    let mut ever_registered = false;

    loop {
        let mut failure_counter = 0u32;
        let mut old_state = None;

        let increase_failures = |counter: &mut u32| {
            *counter += 1;
            if *counter == 10 {
                event_recv.send(LifecycleEvent::FailureThresholdReached(*counter));
            }
        };

        let error = loop {
            let state = match container.state().await {
                Ok(s) => s,
                Err(e) => break e,
            };
            let state_changed = old_state.is_some_and(|s| s != state);
            let entered_running =
                state == PodmanContainerState::Running && (old_state.is_none() || state_changed);

            if old_state.is_none() || state_changed {
                event_recv.send(LifecycleEvent::StateChange(old_state, state));
            }

            // (Re-)register the log stream every time the container becomes
            // Running: Podman's follow-logs request on a container that
            // hasn't started yet ends (EOF) almost immediately rather than
            // waiting, and the log registry silently drops streams that
            // end. Registering only once Running is actually observed
            // avoids that race, and re-registering on every restart
            // recovers logs after a crash/restart too.
            if entered_running {
                if log_handle.is_none() {
                    log_handle = container.take_log_handle();
                }
                if let Some(handle) = &log_handle {
                    let since = ever_registered.then(chrono::Utc::now);
                    ever_registered = true;
                    log_registry.add(
                        application_id,
                        container.name().to_owned(),
                        handle.logs(true, since),
                    );
                }
            }

            let mut timeout = match state {
                PodmanContainerState::Stopped => {
                    // Do not count the initial start as a failure
                    if old_state.is_some() {
                        increase_failures(&mut failure_counter);
                    }

                    if old_state.is_some_and(|s| s == PodmanContainerState::Running) {
                        event_recv.send(LifecycleEvent::StoppedUnexpectedly);
                    }

                    if let Err(e) = container.start().await {
                        break e;
                    }

                    event_recv.send(LifecycleEvent::AttemptingStart);
                    Duration::from_secs(10)
                }
                PodmanContainerState::Ambiguous => {
                    increase_failures(&mut failure_counter);
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
                    // Podman refuses to delete a still-running container
                    // (without --force), so stop it first.
                    if let Err(e) = container.stop().await {
                        event_recv.send(LifecycleEvent::FatalError(e));
                    }
                    if let Err(e) = container.destroy().await {
                        event_recv.send(LifecycleEvent::FatalError(e));
                    }
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
    StoppedUnexpectedly,
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
                info!("Application {} has state {:?}", self.app_name, state)
            }
            LifecycleEvent::StateChange(Some(old_state), new_state) => info!(
                "Application {} changed state: {:?} -> {:?}",
                self.app_name, old_state, new_state
            ),
            LifecycleEvent::StoppedUnexpectedly => {
                warn!("Application {} stopped unexpectedly", self.app_name)
            }
            LifecycleEvent::FailureThresholdReached(failures) => warn!(
                "Application {} seems to have trouble starting (encountered {} failures)",
                self.app_name, failures
            ),
            LifecycleEvent::AttemptingStart => debug!("Trying to start {}", self.app_name),
            LifecycleEvent::FatalError(e) => error!("{:?}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use futures_util::StreamExt;

    use super::{LifecycleEvent::*, *};
    use crate::podman::PodmanContainerState::{Ambiguous, Running, Stopped};

    macro_rules! assert_rcv {
        ($rx:expr, None) => {
            assert!($rx.recv().await.is_none());
        };
        ($rx:expr, $target:pat) => {
            assert!(matches!($rx.recv().await, Some($target)));
        };
    }

    struct ChannelEventReceiver {
        tx: tokio::sync::mpsc::UnboundedSender<LifecycleEvent>,
    }

    impl EventReceiver for ChannelEventReceiver {
        fn send(&self, event: LifecycleEvent) {
            self.tx.send(event).unwrap()
        }
    }

    struct NoopLogHandle;

    impl crate::podman::PodmanLogHandle for NoopLogHandle {
        fn logs(
            &self,
            _follow: bool,
            _since: Option<chrono::DateTime<chrono::Utc>>,
        ) -> futures_util::stream::BoxStream<'static, anyhow::Result<crate::podman::LogChunk>>
        {
            futures_util::stream::empty().boxed()
        }
    }

    #[tokio::test]
    async fn test_application_working() {
        struct MockContainer(PodmanContainerState);

        #[async_trait]
        impl PodmanContainer for MockContainer {
            async fn start(&mut self) -> anyhow::Result<()> {
                self.0 = PodmanContainerState::Running;
                Ok(())
            }
            async fn stop(&mut self) -> anyhow::Result<()> {
                self.0 = PodmanContainerState::Stopped;
                Ok(())
            }
            async fn destroy(self) -> anyhow::Result<()> {
                Ok(())
            }
            async fn state(&self) -> anyhow::Result<PodmanContainerState> {
                Ok(self.0)
            }
            async fn wait_for_state_change(
                &self,
                current: PodmanContainerState,
            ) -> anyhow::Result<()> {
                if current == self.0 {
                    panic!("Waiting forever in test");
                }
                Ok(())
            }

            type LogHandle = NoopLogHandle;
            fn take_log_handle(&mut self) -> Option<Self::LogHandle> {
                Some(NoopLogHandle)
            }
            fn name(&self) -> &str {
                "testname"
            }
        }

        impl PodmanImageInfo for MockContainer {
            fn reference(&self) -> &str {
                "docker.io/library/alpine:latest"
            }
            fn digest(&self) -> &str {
                "unknown"
            }
        }

        let container = MockContainer(PodmanContainerState::Stopped);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
            crate::podman::log_registry::AppLogRegistry::noop(),
            0,
        ));

        assert_rcv!(rx, StateChange(None, Stopped));
        assert_rcv!(rx, AttemptingStart);
        assert_rcv!(rx, StateChange(Some(Stopped), Running));

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
        assert_rcv!(rx, None);
    }

    #[tokio::test]
    async fn test_application_not_starting() {
        struct MockContainer;

        #[async_trait]
        impl PodmanContainer for MockContainer {
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
            async fn wait_for_state_change(
                &self,
                current: PodmanContainerState,
            ) -> anyhow::Result<()> {
                if current == PodmanContainerState::Stopped {
                    tokio::time::sleep(Duration::MAX).await;
                }
                Ok(())
            }
            type LogHandle = NoopLogHandle;
            fn take_log_handle(&mut self) -> Option<Self::LogHandle> {
                Some(NoopLogHandle)
            }
            fn name(&self) -> &str {
                "testname"
            }
        }

        impl PodmanImageInfo for MockContainer {
            fn reference(&self) -> &str {
                "docker.io/library/alpine:latest"
            }
            fn digest(&self) -> &str {
                "unknown"
            }
        }

        let container = MockContainer;

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
            crate::podman::log_registry::AppLogRegistry::noop(),
            0,
        ));

        assert_rcv!(rx, StateChange(None, Stopped));

        // Make sure after some amount of failures the system throws an error
        for i in 0.. {
            match rx.recv().await.unwrap() {
                AttemptingStart => assert!(i < 100),
                FailureThresholdReached(failures) => {
                    assert_eq!(i, failures);
                    break;
                }
                _ => panic!(),
            }
        }

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
    }

    #[tokio::test]
    async fn test_application_ambiguous() {
        struct MockContainer(PodmanContainerState);

        #[async_trait]
        impl PodmanContainer for MockContainer {
            async fn start(&mut self) -> anyhow::Result<()> {
                self.0 = PodmanContainerState::Ambiguous;
                Ok(())
            }
            async fn stop(&mut self) -> anyhow::Result<()> {
                self.0 = PodmanContainerState::Stopped;
                Ok(())
            }
            async fn destroy(self) -> anyhow::Result<()> {
                Ok(())
            }
            async fn state(&self) -> anyhow::Result<PodmanContainerState> {
                Ok(self.0)
            }
            async fn wait_for_state_change(
                &self,
                current: PodmanContainerState,
            ) -> anyhow::Result<()> {
                if current == self.0 {
                    tokio::time::sleep(Duration::MAX).await;
                }
                Ok(())
            }
            type LogHandle = NoopLogHandle;
            fn take_log_handle(&mut self) -> Option<Self::LogHandle> {
                Some(NoopLogHandle)
            }
            fn name(&self) -> &str {
                "testname"
            }
        }

        impl PodmanImageInfo for MockContainer {
            fn reference(&self) -> &str {
                "docker.io/library/alpine:latest"
            }
            fn digest(&self) -> &str {
                "unknown"
            }
        }

        let container = MockContainer(PodmanContainerState::Stopped);

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
            crate::podman::log_registry::AppLogRegistry::noop(),
            0,
        ));

        assert_rcv!(rx, StateChange(None, Stopped));
        assert_rcv!(rx, AttemptingStart);
        assert_rcv!(rx, StateChange(Some(Stopped), Ambiguous));
        assert_rcv!(rx, FailureThresholdReached(_));

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
    }

    #[tokio::test]
    async fn test_application_crashing() {
        struct MockContainer(Mutex<PodmanContainerState>);

        #[async_trait]
        impl PodmanContainer for MockContainer {
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
            async fn wait_for_state_change(
                &self,
                current: PodmanContainerState,
            ) -> anyhow::Result<()> {
                // Apparently tokio::test uses a single-threaded runtime, so
                // the test would never complete without explicitly yielding
                tokio::task::yield_now().await;

                match current {
                    PodmanContainerState::Running => {
                        *self.0.lock().unwrap() = PodmanContainerState::Stopped;
                    }
                    x if x == *self.0.lock().unwrap() => {
                        panic!("Waiting forever in test");
                    }
                    _ => {}
                }
                Ok(())
            }
            type LogHandle = NoopLogHandle;
            fn take_log_handle(&mut self) -> Option<Self::LogHandle> {
                Some(NoopLogHandle)
            }
            fn name(&self) -> &str {
                "testname"
            }
        }

        impl PodmanImageInfo for MockContainer {
            fn reference(&self) -> &str {
                "docker.io/library/alpine:latest"
            }
            fn digest(&self) -> &str {
                "unknown"
            }
        }

        let container = MockContainer(Mutex::new(PodmanContainerState::Stopped));

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            Arc::new(tokio::sync::Notify::const_new()),
            crate::podman::log_registry::AppLogRegistry::noop(),
            0,
        ));

        assert_rcv!(rx, StateChange(None, Stopped));

        for i in 1.. {
            assert_rcv!(rx, AttemptingStart);
            assert_rcv!(rx, StateChange(Some(Stopped), Running));
            assert_rcv!(rx, StateChange(Some(Running), Stopped));

            match rx.recv().await.unwrap() {
                StoppedUnexpectedly => assert!(i < 100),
                FailureThresholdReached(x) if x == i => break,
                x => panic!("Got unexpected event {:?}", x),
            }
        }

        lifecycle_loop.abort();
        assert!(lifecycle_loop.await.is_err());
    }

    #[tokio::test]
    async fn test_removal_stops_running_container_before_destroying() {
        // Regression test: Podman refuses to delete a still-running
        // container without --force, so destroy() alone left the container
        // orphaned (running forever) whenever it was removed while Running.
        struct MockContainer {
            state: Mutex<PodmanContainerState>,
        }

        #[async_trait]
        impl PodmanContainer for MockContainer {
            async fn start(&mut self) -> anyhow::Result<()> {
                *self.state.lock().unwrap() = Running;
                Ok(())
            }
            async fn stop(&mut self) -> anyhow::Result<()> {
                *self.state.lock().unwrap() = Stopped;
                Ok(())
            }
            async fn destroy(self) -> anyhow::Result<()> {
                // Mirrors Podman's real behaviour: refuses to remove a
                // container that is still Running.
                if *self.state.lock().unwrap() == Running {
                    anyhow::bail!("cannot remove a running container");
                }
                Ok(())
            }
            async fn state(&self) -> anyhow::Result<PodmanContainerState> {
                Ok(*self.state.lock().unwrap())
            }
            async fn wait_for_state_change(
                &self,
                _current: PodmanContainerState,
            ) -> anyhow::Result<()> {
                std::future::pending().await
            }
            type LogHandle = NoopLogHandle;
            fn take_log_handle(&mut self) -> Option<Self::LogHandle> {
                Some(NoopLogHandle)
            }
            fn name(&self) -> &str {
                "testname"
            }
        }

        impl PodmanImageInfo for MockContainer {
            fn reference(&self) -> &str {
                "docker.io/library/alpine:latest"
            }
            fn digest(&self) -> &str {
                "unknown"
            }
        }

        let container = MockContainer {
            state: Mutex::new(Stopped),
        };

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let delete_notifier = Arc::new(tokio::sync::Notify::const_new());

        let lifecycle_loop = tokio::spawn(run_lifecycle_loop(
            container,
            ChannelEventReceiver { tx },
            delete_notifier.clone(),
            crate::podman::log_registry::AppLogRegistry::noop(),
            0,
        ));

        assert_rcv!(rx, StateChange(None, Stopped));
        assert_rcv!(rx, AttemptingStart);
        assert_rcv!(rx, StateChange(Some(Stopped), Running));

        delete_notifier.notify_one();

        // No FatalError from a failed destroy() must be observed: the loop
        // must stop the container before destroying it.
        assert_rcv!(rx, None);
        lifecycle_loop.await.unwrap();
    }
}
