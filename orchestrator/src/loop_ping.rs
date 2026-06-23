//! Logic to repeatedly signal aliveness to the API

use std::{sync::Arc, time::Duration};

use tracing::warn;

use crate::api_client::ApiClient;

/// Repeatedly send pings to the API to signal aliveness
pub async fn run_ping_main_loop(api_client: Arc<ApiClient>, interval: Duration) -> ! {
    let mut update_interval = tokio::time::interval(interval);
    // Prevent bursting should an update cycle take longer than expected
    update_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        update_interval.tick().await;

        if let Err(e) = api_client.send_ping().await {
            warn!("Aliveness report failed: {}", e);
        }
    }
}
