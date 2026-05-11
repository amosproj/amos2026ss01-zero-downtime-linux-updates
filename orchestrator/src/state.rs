use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config_loader::Settings;

#[derive(Debug, Clone)]
pub struct OsState {
    #[expect(unused)]
    pub update_pending: bool, // when an update is pending (updated but not yet rebooted)
    pub booted_image: String, // the current version and tag of the running image
    #[expect(unused)]
    pub update_ostree_commit: Option<String>, // if update available -> the image tag for the update
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub app_id: String,  // the podman/docker image name
    pub version: String, // the podman/docker image tag
    #[expect(unused)]
    pub updating: bool,
    // TODO: add more app info as needed (e.g. run args, compose file)
}

#[derive(Debug, Clone)]
pub struct AgentState {
    pub self_version: String,
    pub config: Settings,

    pub os_state: Arc<Mutex<OsState>>,
    pub apps_state: Arc<Mutex<Vec<AppState>>>,
}

impl AgentState {
    pub fn new(
        version: impl Into<String>,
        config: Settings,
        initial_os_state: OsState,
        inital_apps_state: Vec<AppState>,
    ) -> Self {
        Self {
            self_version: version.into(),
            config,
            os_state: Arc::new(Mutex::new(initial_os_state)),
            apps_state: Arc::new(Mutex::new(inital_apps_state)),
        }
    }
}
