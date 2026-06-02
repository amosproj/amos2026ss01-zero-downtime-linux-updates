use crate::download_manager::DownloadManager;
use crate::state::{AgentState, OsState};
use crate::update_check::{CheckForUpdate, UpdateDecision};
use crate::util::bootc_wrapper::Bootc;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

pub async fn run_os_tree_main_loop(
    agent_state: AgentState,
    client: Arc<Bootc>,
    download_manager: Arc<DownloadManager>,
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

        let decision = match update_checker.check_os(&bootc_status).await {
            Ok(d) => d,
            Err(e) => {
                error!("OS update check failed: {:?}", e);
                continue;
            }
        };
        let booted_checksum = bootc_status
            .booted
            .as_ref()
            .expect("No booted OS found")
            .checksum
            .clone();

        let current_os_state = OsState {
            update_pending: bootc_status.staged.is_some(),
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

            if let Err(e) = client.apply().await {
                error!("Critical: switch succeeded but apply failed: {:?}", e);
            }
        }
        Err(e) => {
            error!("bootc switch failed: {:?}", e);
        }
    }
}
