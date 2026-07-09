use std::cell::RefCell;

tokio::task_local! {
    pub static CURRENT_USER: RefCell<Option<String>>;
}

