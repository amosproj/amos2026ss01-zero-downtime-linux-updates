use std::cell::RefCell;

tokio::task_local! {
    pub static CURRENT_USER: RefCell<Option<String>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sets_and_reads_user() {
        let user = RefCell::new(Some("test-user".to_string()));
        CURRENT_USER
            .scope(user, async {
                let u = current_user();
                assert_eq!(u, Some("test-user".to_string()));
            })
            .await;
    }

    #[tokio::test]
    async fn none_outside_scope() {
        let u = current_user();
        assert_eq!(u, None);
    }

    #[tokio::test]
    async fn survives_await() {
        let user = RefCell::new(Some("await-user".to_string()));
        CURRENT_USER
            .scope(user, async {
                tokio::task::yield_now().await;
                let u = current_user();
                assert_eq!(u, Some("await-user".to_string()));
            })
            .await;
    }
}
