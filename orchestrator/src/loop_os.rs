use crate::download_manager::DownloadManager;
use crate::state::{AgentState, OsState};
use crate::util::bootc_wrapper::Bootc;
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

pub async fn run_os_tree_main_loop(
    agent_state: AgentState,
    client: Arc<Bootc>,
    download_manager: Arc<DownloadManager>,
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

        let expected_os_version = match download_manager.get_expected_os_version().await {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to get expected OS version from API: {:?}", e);
                continue;
            }
        };

        let booted_checksum = bootc_status.booted.unwrap().checksum.clone();
        let target_commit = expected_os_version.commit_hash;
        if booted_checksum == target_commit
            && let Err(e) = download_manager
                .report_os_assignment(expected_os_version.id)
                .await
        {
            warn!("Failed to report OS assignment: {:?}", e);
        }

        let host_os_state = OsState {
            update_pending: bootc_status.staged.is_some(),
            booted_image: booted_checksum.clone(),
            update_ostree_commit: bootc_status.staged.map(|s| s.checksum),
        };

        {
            let mut current_state = agent_state.os_state.lock().await;
            *current_state = host_os_state;
        }

        handle_bootc(&client, &booted_checksum, &target_commit).await;
    }
}

async fn handle_bootc(client: &Bootc, booted_checksum: &str, target_commit: &str) {
    info!("Checking for OS update");

    if booted_checksum != target_commit {
        info!(
            "New image detected! Current: {} -> Target: {}",
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
    } else {
        info!("System is up to date.");
    }
}
