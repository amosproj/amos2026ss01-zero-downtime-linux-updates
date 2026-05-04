pub struct OsState {
    running_ostree_commit: String,
}

pub struct AppState {
    app_id: String,
    running_version: String,
}

pub struct AgentState {
    self_version: String,
    os_state: OsState,
    apps: Vec<AgentState>,
}
