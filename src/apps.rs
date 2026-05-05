use crate::state::AppState;

pub fn get_initial_state() -> Vec<AppState> {
    vec![AppState {
        app_id: String::from("data_collector"),
        running_version: String::from("v1.0.1"),
        update_version: Option::None,
    }]
}
