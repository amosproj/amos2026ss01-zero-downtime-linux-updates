use std::time::Duration;
use std::process::Command;

use log::info;
use tokio::time::interval;

use crate::state::{AgentState, OsState};

pub fn get_initial_os_state() -> OsState {
    OsState {
        update_pending: false,
        booted_image: String::from("current_latest"),
        update_ostree_commit: Option::None,
    }
}

pub async fn run_os_tree_main_loop(agent_state: AgentState) {
    let mut update_interval = interval(Duration::from_secs(
        agent_state.config.poll_interval_secs.into(),
    ));

    loop {
        update_interval.tick().await;

        let target_os_state = get_target_os_state().await;
        let host_os_state = get_host_os_state().await;

        {
            let mut current_state = agent_state.os_state.lock().await;
            *current_state = host_os_state.clone();
        }

        handle_os_tree(agent_state.os_state.lock().await.clone(), target_os_state).await;
    }
}

async fn get_target_os_state() -> OsState {
    OsState {
        update_pending: false,
        booted_image: String::from("current_latest_new"),
        update_ostree_commit: Some(String::from("next_latest")),
    }
}

async fn get_host_os_state() -> OsState {
    OsState {
        update_pending: false,
        booted_image: String::from("current_latest"),
        update_ostree_commit: Option::None,
    }
}

async fn handle_os_tree(current_state: OsState, target_state: OsState) {
    info!("Checking for OS update");

    if current_state.booted_image != target_state.booted_image {
        run_update_command(&current_state, &target_state).await;
    }
}

// requires root privileges
async fn run_update_command(current_state: &OsState, target_state: &OsState) {
    info!("Triggering OS update to commit: {:?}", target_state.booted_image);

    let status = Command::new("sudo")
        .args(["bootc", "upgrade"])
        .status();

    match status {
        Ok(s) if s.success() => {
            info!("OS update staged successfully.");
            reboot_device();
        }
        Ok(s) => error!("bootc upgrade failed with status: {}", s),
        Err(e) => error!("Failed to execute bootc: {}", e),
    }
}

fn reboot_device() {
    info!("Rebooting system to apply OS update...");
    let _ = Command::new("systemctl").arg("reboot").status();
}
