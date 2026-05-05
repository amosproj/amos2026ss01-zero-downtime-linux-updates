use crate::state::OsState;

pub fn get_inital_os_state() -> OsState {
    OsState {
        update_pending: false,
        running_ostree_commit: String::from("current_latest"),
        update_ostree_commit: Option::None,
    }
}
