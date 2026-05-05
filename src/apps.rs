use std::{collections::HashMap, time::Duration};

use log::{info, warn};
use tokio::time::interval;

use crate::state::{AgentState, AppState};

pub fn get_initial_apps_state() -> Vec<AppState> {
    vec![AppState {
        app_id: String::from("data_collector"),
        version: String::from("v1.0.1"),
        updating: false,
    }]
}

pub async fn run_apps_main_loop(agent_state: AgentState) {
    let mut update_interval = interval(Duration::from_secs(
        agent_state.config.poll_interval_secs.into(),
    ));

    loop {
        // run loop only as often as defined in the config
        update_interval.tick().await;

        // TODO: Placeholder: get app information from api
        let target_app_state = vec![AppState {
            app_id: String::from("data_collector"),
            version: String::from("v1.0.2"),
            updating: false,
        }];

        // TODO: Placeholder: get app states from host (podman)
        let host_app_state = vec![AppState {
            app_id: String::from("data_collector"),
            version: String::from("v1.0.1"),
            updating: false,
        }];

        {
            // set the host_app_state to the global app_state
            let mut current_state = agent_state.apps_state.lock().await;
            *current_state = host_app_state;
        }

        handle_apps(agent_state.clone(), target_app_state).await;
    }
}

pub async fn handle_apps(agent_state: AgentState, target_state: Vec<AppState>) {
    let current_state_snapshot = {
        let current_state = agent_state.apps_state.lock().await;
        current_state.clone()
    };

    let current_by_id = index_apps(current_state_snapshot, "current_state");
    let target_by_id = index_apps(target_state, "target_state");

    // find missing/to_update containers and create/update them
    for (app_id, target_app) in &target_by_id {
        match current_by_id.get(app_id) {
            Some(current_app) if current_app.version == target_app.version => {}
            Some(current_app) => {
                info!(
                    "Updating container {} from {} to {}",
                    app_id, current_app.version, target_app.version
                );
                update_container(
                    app_id,
                    Some(current_app.version.as_str()),
                    target_app.version.as_str(),
                )
                .await;
            }
            None => {
                info!(
                    "Creating container {} at version {}",
                    app_id, target_app.version
                );
                create_container(app_id, target_app.version.as_str()).await;
            }
        }
    }

    // find containers to delete and delete them
    for (app_id, current_app) in current_by_id {
        if !target_by_id.contains_key(&app_id) {
            info!(
                "Deleting container {} at version {}",
                app_id, current_app.version
            );
            delete_container(&app_id, current_app.version.as_str()).await;
        }
    }
}

fn index_apps(apps: Vec<AppState>, label: &str) -> HashMap<String, AppState> {
    let mut by_id = HashMap::new();
    for app in apps {
        if by_id.contains_key(&app.app_id) {
            warn!("Duplicate app_id in {} ignored: {}", label, app.app_id);
            continue;
        }
        by_id.insert(app.app_id.clone(), app);
    }
    by_id
}

async fn update_container(app_id: &str, from_version: Option<&str>, to_version: &str) {
    let _ = (app_id, from_version, to_version);
}

async fn create_container(app_id: &str, to_version: &str) {
    let _ = (app_id, to_version);
}

async fn delete_container(app_id: &str, from_version: &str) {
    let _ = (app_id, from_version);
}
