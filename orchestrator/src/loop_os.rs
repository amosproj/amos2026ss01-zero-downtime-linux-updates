use crate::api_client::ApiClient;
use crate::state::{AgentState, OsState};
use crate::update_check::{CheckForUpdate, UpdateDecision};
use crate::util::bootc_wrapper::Bootc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{error, info, warn};

pub async fn run_os_tree_main_loop(
    agent_state: AgentState,
    client: Arc<Bootc>,
    download_manager: Arc<ApiClient>,
    update_checker: Arc<dyn CheckForUpdate>,
) {
    let mut update_interval = interval(Duration::from_secs(
        agent_state.config.poll_interval_secs.into(),
    ));

    loop {
        update_interval.tick().await;

        let bootc_status = match client.status().await {
            Ok(status) => status,
            Err(e) => {
                error!("Failed to fetch live bootc status: {:?}", e);
                continue;
            }
        };

        let booted_checksum = match bootc_status.booted.as_ref() {
            Some(booted) => booted.checksum.clone(),
            None => {
                warn!("bootc status reports no booted deployment; skipping OS update cycle");
                continue;
            }
        };

        let decision = match update_checker.check_os(&bootc_status).await {
            Ok(d) => d,
            Err(e) => {
                error!("OS update check failed: {:?}", e);
                continue;
            }
        };

        let update_pending = bootc_status.staged.is_some();
        let current_os_state = OsState {
            update_pending,
            booted_image: booted_checksum.clone(),
            update_ostree_commit: bootc_status.staged.map(|s| s.checksum),
        };

        {
            let mut current_state = agent_state.os_state.lock().await;
            *current_state = current_os_state;
        }

        match decision {
            UpdateDecision::UpToDate { target } => {
                info!(
                    "OS is up to date (target #{} {})",
                    target.id, target.commit_hash
                );
                if let Err(e) = download_manager
                    .report_current_os_assignment(target.id)
                    .await
                {
                    warn!("Failed to report OS assignment: {:?}", e);
                }
            }
            UpdateDecision::UpdateRequired { reasons, target } => {
                for reason in &reasons {
                    info!("{}", reason);
                }
                if update_pending {
                    warn!(
                        "An update is already staged but the target has changed; \
                         re-staging on top of the existing staged deployment",
                    );
                }
                handle_bootc(&client, &booted_checksum, &target.commit_hash).await;
            }
        }
    }
}

async fn handle_bootc(client: &Bootc, booted_checksum: &str, target_commit: &str) {
    info!(
        "Switching OS image: current {} -> target {}",
        booted_checksum, target_commit
    );

    match client.switch(target_commit).await {
        Ok(()) => {
            info!("bootc switch staged successfully. Applying and rebooting...");

            // Last log line before the reboot
            match client.apply().await {
                Ok(()) => info!(target_commit, "bootc apply succeeded; reboot imminent",),
                Err(e) => error!("Critical: switch succeeded but apply failed: {:?}", e),
            }
        }
        Err(e) => {
            error!("bootc switch failed: {:?}", e);
        }
    }
}
