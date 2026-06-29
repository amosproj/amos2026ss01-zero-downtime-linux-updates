//! Logic to repeatedly check for OS updates and apply them

use crate::util::bootc_wrapper::Bootc;
use crate::{api_client::ApiClient, util::bootc_wrapper::BootcStatus};
use anyhow::Context;
use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info, warn};

/// Repeatedly check for OS updates and apply them.
pub async fn run_os_main_loop(
    mut os_state: OsState,
    bootc: Arc<Bootc>,
    api_client: Arc<ApiClient>,
    os_upgrade_in_progress: Arc<std::sync::atomic::AtomicBool>,
    poll_interval: Duration,
    deferred_timer: Duration,
) -> ! {
    let mut update_interval = tokio::time::interval(poll_interval);
    // Prevent bursting should an update cycle take longer than expected
    update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        update_interval.tick().await;

        if let Err(e) = try_update(
            &mut os_state,
            &bootc,
            &api_client,
            &os_upgrade_in_progress,
            deferred_timer,
        )
        .await
        {
            error!("{}", e.context("OS update cycle failed"));
        }
    }
}

async fn try_update(
    state: &mut OsState,
    bootc: &Arc<Bootc>,
    api_client: &ApiClient,
    os_upgrade_in_progress: &Arc<std::sync::atomic::AtomicBool>,
    deferred_timer: Duration,
) -> anyhow::Result<()> {
    let status = bootc.status().await?;

    *state = match OsState::new(status) {
        Some(s) => s,
        None => {
            warn!("bootc status reports no booted deployment; skipping OS update cycle");
            return Ok(());
        }
    };

    let (target, immediate) = api_client.get_target_os_version().await?;
    if state.booted_image == target.commit_hash {
        // Yay, we are up to date!
        api_client.report_current_os_assignment(target.id).await?;
        return Ok(());
    }

    if state.staged_image.as_deref() == Some(&target.commit_hash) {
        if immediate {
            // It was staged deferred before, but now the database flag changed to 'immediate'
            info!("Target image is already staged. 'immediate' flag is true; forcing reboot.");
            bootc
                .upgrade_from_downloaded(true)
                .await
                .context("Immediate apply failed")?;
        } else {
            // Normal case: It's already staged and the timer is already running in the background.
            tracing::debug!(
                "Target image {} is already staged. Waiting for reboot.",
                target.commit_hash
            );
        }
        return Ok(());
    }

    if state.update_pending {
        warn!(
            "An update is already staged but the target has changed; \
                re-staging on top of the existing staged deployment",
        );
    }

    info!(
        "Switching OS image: current {} -> target {}, immediate = {}",
        state.booted_image, target.commit_hash, immediate
    );

    if immediate {
        info!(
            "Switching OS image immediately: {} -> {}",
            state.booted_image, target.commit_hash
        );
        info!("Locking application loops and forcing immediate OS update...");
        os_upgrade_in_progress.store(true, std::sync::atomic::Ordering::SeqCst);
        bootc.switch(&target.commit_hash).await?;
        bootc.apply().await.context("Immediate apply failed")?;
    } else {
        info!(
            "Staging OS image deferred: {} -> {}",
            state.booted_image, target.commit_hash
        );

        bootc
            .upgrade_download_only()
            .await
            .context("Deferred staging failed")?;

        let timer_bootc = Arc::clone(bootc);
        let timer_upgrade_flag = Arc::clone(os_upgrade_in_progress);

        tokio::spawn(async move {
            info!("Started countdown for deferred OS update.");
            // User reboot automatically cleans up the timer, leading to a switch
            tokio::time::sleep(deferred_timer).await;

            info!("Timer expired! Locking application updates and executing OS reboot...");
            timer_upgrade_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Err(e) = timer_bootc.upgrade_from_downloaded(true).await {
                error!("Failed to apply deferred update after timer: {}", e);
            }
        });
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct OsState {
    update_pending: bool, // When an update is pending (updated but not yet rebooted)
    booted_image: String, // The version and tag of the running image
    staged_image: Option<String>, // The checksum of the staged update, if any
}

impl OsState {
    pub fn new(status: BootcStatus) -> Option<Self> {
        Some(Self {
            update_pending: status.staged.is_some(),
            booted_image: status.booted?.checksum,
            staged_image: status.staged.map(|deployment| deployment.checksum),
        })
    }
}
