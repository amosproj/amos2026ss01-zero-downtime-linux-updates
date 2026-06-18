use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{application::Application};

#[derive(Debug, Clone, Deserialize)]
pub struct OsState {
    pub update_pending: bool, // when an update is pending (updated but not yet rebooted)
    pub booted_image: String, // the current version and tag of the running image
    pub update_ostree_commit: Option<String>, // if update available -> the image tag for the update
}

#[derive(Debug, Clone)]
pub struct AgentState {
    pub self_version: String,
    pub config: Arc<Settings>,

    pub os_state: Arc<Mutex<OsState>>,
    pub apps_state: Arc<Mutex<Vec<Application>>>,
}

impl AgentState {
    pub fn new(
        version: impl Into<String>,
        config: Arc<Settings>,
        initial_os_state: OsState,
        inital_apps_state: Vec<Application>,
    ) -> Self {
        Self {
            self_version: version.into(),
            config,
            os_state: Arc::new(Mutex::new(initial_os_state)),
            apps_state: Arc::new(Mutex::new(inital_apps_state)),
        }
    }
}
