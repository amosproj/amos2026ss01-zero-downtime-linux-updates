use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use amos_common::entities::ApplicationConfig;
use log::{error, info, warn};
use tokio::time::interval;

use crate::download_manager::DownloadManager;
use crate::inventory::{ApplicationInfo, CollectionResult, collect_inventory};
use crate::state::{AgentState, AppState};
use crate::update_check::{CheckForUpdate, UpdateDecision};
use crate::util::bootc_wrapper::Bootc;
use crate::util::executer::Executer;

pub fn get_initial_apps_state() -> Vec<AppState> {
    // Real state is populated from the first inventory collection in the loop.
    Vec::new()
}

pub async fn run_apps_main_loop(
    agent_state: AgentState,
    download_manager: Arc<DownloadManager>,
    update_checker: Arc<dyn CheckForUpdate>,
    bootc: Arc<Bootc>,
    exec: Arc<dyn Executer>,
) {
    let mut update_interval = interval(Duration::from_secs(
        agent_state.config.poll_interval_secs.into(),
    ));

    loop {
        // run loop only as often as defined in the config
        update_interval.tick().await;

        let inventory = match collect_inventory(&bootc, exec.as_ref()).await {
            Ok(inv) => inv,
            Err(e) => {
                error!("Failed to collect inventory for apps loop: {:?}", e);
                continue;
            }
        };

        {
            // set the current_app_state to the global app_state
            let mut current_state = agent_state.apps_state.lock().await;
            *current_state = apps_state_from_inventory(&inventory.applications);
        }

        let decision = match update_checker.check_apps(&inventory.applications).await {
            Ok(Some(d)) => d,
            Ok(None) => continue, // local inventory unavailable, skip cycle
            Err(e) => {
                error!("Apps update check failed: {:?}", e);
                continue;
            }
        };

        match decision {
            UpdateDecision::UpToDate { target } => {
                info!("Apps up to date ({} configs assigned)", target.len());
                report_running_configs(&download_manager, &target).await;
            }
            UpdateDecision::UpdateRequired { reasons, target } => {
                for reason in &reasons {
                    info!("{}", reason);
                }
                reconcile_containers(&inventory.applications, &target).await;
            }
        }
    }
}

fn apps_state_from_inventory(
    applications: &CollectionResult<Vec<ApplicationInfo>>,
) -> Vec<AppState> {
    match applications {
        CollectionResult::Ok(apps) => apps
            .iter()
            .map(|a| AppState {
                app_id: a.app_name.clone(),
                version: a.app_version.clone(),
                updating: false,
            })
            .collect(),
        CollectionResult::Unavailable { .. } => Vec::new(),
    }
}

async fn report_running_configs(
    download_manager: &DownloadManager,
    target: &[ApplicationConfig::Model],
) {
    for cfg in target {
        if let Err(e) = download_manager
            .report_current_application_assignment(cfg.id)
            .await
        {
            warn!(
                "Failed to report application assignment for config #{}: {e:?}",
                cfg.id
            );
        }
    }
}

async fn reconcile_containers(
    current: &CollectionResult<Vec<ApplicationInfo>>,
    target: &[ApplicationConfig::Model],
) {
    let current_images: HashSet<String> = match current {
        CollectionResult::Ok(apps) => apps.iter().map(image_key).collect(),
        CollectionResult::Unavailable { .. } => HashSet::new(),
    };

    let target_images: HashSet<&String> = target.iter().map(|c| &c.image).collect();

    for cfg in target {
        if !current_images.contains(&cfg.image) {
            info!("Creating container for image {}", cfg.image);
            create_container(&cfg.image).await;
        }
    }

    for img in &current_images {
        if !target_images.contains(img) {
            info!("Deleting container for image {}", img);
            delete_container(img).await;
        }
    }
}

fn image_key(app: &ApplicationInfo) -> String {
    if app.app_version.is_empty() {
        app.app_name.clone()
    } else {
        format!("{}:{}", app.app_name, app.app_version)
    }
}

async fn create_container(image: &str) {
    let _ = image;
}

async fn delete_container(image: &str) {
    let _ = image;
}
