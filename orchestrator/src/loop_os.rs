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
    bootc: Bootc,
    api_client: Arc<ApiClient>,
    poll_interval: Duration,
) -> ! {
    let mut update_interval = tokio::time::interval(poll_interval);
    // Prevent bursting should an update cycle take longer than expected
    update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        update_interval.tick().await;

        if let Err(e) = try_update(&mut os_state, &bootc, &api_client).await {
            error!("{}", e.context("OS update cycle failed"));
        }
    }
}

async fn try_update(
    state: &mut OsState,
    bootc: &Bootc,
    api_client: &ApiClient,
) -> anyhow::Result<()> {
    let status = bootc.status().await?;

    *state = match OsState::new(status) {
        Some(s) => s,
        None => {
            warn!("bootc status reports no booted deployment; skipping OS update cycle");
            return Ok(());
        }
    };

    let target = api_client.get_target_os_version().await?;
    if state.booted_image == target.commit_hash {
        // Yay, we are up to date!
        api_client.report_current_os_assignment(target.id).await?;
        return Ok(());
    }

    if state.update_pending {
        warn!(
            "An update is already staged but the target has changed; \
                re-staging on top of the existing staged deployment",
        );
    }

    info!(
        "Switching OS image: current {} -> target {}",
        state.booted_image, target.commit_hash
    );

    bootc.switch(&target.commit_hash).await?;
    info!("bootc switch staged successfully. Applying and rebooting...");

    bootc
        .apply()
        .await
        .context("Critical: switch succeeded but apply failed")?;
    info!("bootc apply succeeded; reboot imminent");

    Ok(())
}

#[derive(Debug, Clone)]
pub struct OsState {
    update_pending: bool, // when an update is pending (updated but not yet rebooted)
    booted_image: String, // the current version and tag of the running image
}

impl OsState {
    pub fn new(status: BootcStatus) -> Option<Self> {
        Some(Self {
            update_pending: status.staged.is_some(),
            booted_image: status.booted?.checksum,
        })
    }
}
