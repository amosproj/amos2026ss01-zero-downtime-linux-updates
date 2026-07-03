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
    os_switch_in_progress: Arc<std::sync::atomic::AtomicBool>,
    poll_interval: Duration,
    deferred_timer: Duration,
) -> ! {
    let mut update_interval = tokio::time::interval(poll_interval);
    // Prevent bursting should an update cycle take longer than expected
    update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        update_interval.tick().await;

        if let Err(e) = try_switch(
            &mut os_state,
            &bootc,
            &api_client,
            &os_switch_in_progress,
            deferred_timer,
        )
        .await
        {
            error!("{:?}", e.context("OS update cycle failed"));
        }
    }
}

async fn try_switch(
    state: &mut OsState,
    bootc: &Arc<Bootc>,
    api_client: &ApiClient,
    os_switch_in_progress: &Arc<std::sync::atomic::AtomicBool>,
    deferred_timer: Duration,
) -> anyhow::Result<()> {
    let status = bootc.status().await?;

    let current_countdown = state.countdown_started;

    *state = match OsState::new(status) {
        Some(mut s) => {
            s.countdown_started = current_countdown;
            s
        }
        None => {
            warn!("bootc status reports no booted deployment; skipping OS update cycle");
            return Ok(());
        }
    };

    let (target, immediate) = api_client.get_target_os_version().await?;

    if state.booted_checksum == target.commit_hash
        || state.booted_image_ref.as_deref() == Some(target.commit_hash.as_str())
    {
        info!("System is already up to date.");
        api_client.report_current_os_assignment(target.id).await?;
        return Ok(());
    }

    // Allows to assign a ghcr.io link instead of checksum
    let is_target_staged = state.staged_checksum.as_deref() == Some(target.commit_hash.as_str())
        || state.staged_image_ref.as_deref() == Some(target.commit_hash.as_str());

    if is_target_staged {
        if immediate {
            info!("Target image is already staged, flag changed. Forcing immediate reboot.");
            os_switch_in_progress.store(true, std::sync::atomic::Ordering::SeqCst);
            bootc
                .apply()
                .await
                .context("Immediate apply of staged image failed")?;
        } else {
            info!("Target image is staged and counting down. Waiting for timer.");
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
        state.booted_image_ref.as_deref().unwrap_or("unknown"),
        target.commit_hash,
        immediate
    );

    // Handle fresh image targets that haven't been downloaded yet
    if immediate {
        info!(
            "Switching OS image immediately: {} -> {}",
            state.booted_image_ref.as_deref().unwrap_or("unknown"),
            target.commit_hash
        );
        info!("Locking application loops and forcing immediate OS update...");
        // Lock application loops immediately
        os_switch_in_progress.store(true, std::sync::atomic::Ordering::SeqCst);

        bootc.switch(&target.commit_hash).await?;
        bootc.apply().await.context("Immediate apply failed")?;
    } else {
        info!(
            "Staging OS image deferred: {} -> {}",
            state.booted_image_ref.as_deref().unwrap_or("unknown"),
            target.commit_hash
        );

        bootc.switch(&target.commit_hash).await?;

        state.countdown_started = true;

        let timer_bootc = Arc::clone(bootc);
        let timer_upgrade_flag = Arc::clone(os_switch_in_progress);

        let b_state = bootc.status().await?;
        info!("Current OS State right before timer spawn: {:?}", b_state);

        // Defer only the application/reboot phase to the background timer thread
        tokio::spawn(async move {
            info!("Started countdown for deferred OS update.");
            tokio::time::sleep(deferred_timer).await;

            info!("Timer expired! Locking application updates and executing OS reboot...");
            // Lock container updates when the countdown finishes
            timer_upgrade_flag.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Err(e) = timer_bootc.apply().await {
                error!("Failed to apply deferred update after timer: {}", e);
            }
        });
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct OsState {
    update_pending: bool,
    booted_checksum: String,
    booted_image_ref: Option<String>,
    staged_checksum: Option<String>,
    staged_image_ref: Option<String>,
    countdown_started: bool,
}

impl OsState {
    pub fn new(status: BootcStatus) -> Option<Self> {
        let booted = status.booted?;
        Some(Self {
            update_pending: status.staged.is_some(),
            booted_checksum: booted.checksum,
            booted_image_ref: booted.image.map(|i| i.image_ref),
            staged_checksum: status.staged.as_ref().map(|d| d.checksum.clone()),
            staged_image_ref: status.staged.and_then(|d| d.image).map(|i| i.image_ref),
            countdown_started: false,
        })
    }
}
