use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug)]
pub struct OsState {
    pub update_pending: bool, // when an update is pending (updated but not yet rebooted)
    pub running_ostree_commit: String, // the current version and tag of the running image
    pub update_ostree_commit: Option<String>, // if update available -> the image tag for the update
}

#[derive(Debug)]
pub struct AppState {
    pub app_id: String,                 // the podman/docker image name
    pub running_version: String,        // the podman/docker image tag
    pub update_version: Option<String>, // the image tag if an update is available
}

#[derive(Debug, Clone)]
pub struct AgentState {
    self_version: String,

    os_state: Arc<Mutex<OsState>>,
    apps: Arc<Mutex<Vec<AppState>>>,
}

impl AgentState {
    pub fn new(
        version: impl Into<String>,
        initial_os_state: OsState,
        inital_apps_state: Vec<AppState>,
    ) -> Self {
        Self {
            self_version: version.into(),
            os_state: Arc::new(Mutex::new(initial_os_state)),
            apps: Arc::new(Mutex::new(inital_apps_state)),
        }
    }
}
