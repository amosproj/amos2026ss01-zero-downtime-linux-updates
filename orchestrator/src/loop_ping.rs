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

        if let Err(e) = api_client.send_ping(read_uptime_secs().await).await {
            warn!("Aliveness report failed: {}", e);
        }
    }
}

/// Reads system uptime from /proc/uptime; None if unavailable (ping is still sent without it).
async fn read_uptime_secs() -> Option<i64> {
    parse_uptime_secs(&tokio::fs::read_to_string("/proc/uptime").await.ok()?)
}

fn parse_uptime_secs(proc_uptime: &str) -> Option<i64> {
    Some(proc_uptime.split_whitespace().next()?.parse::<f64>().ok()? as i64)
}

#[cfg(test)]
mod tests {
    use super::parse_uptime_secs;

    #[test]
    fn parses_proc_uptime() {
        assert_eq!(parse_uptime_secs("12345.67 45678.90\n"), Some(12345));
        assert_eq!(parse_uptime_secs(""), None);
        assert_eq!(parse_uptime_secs("garbage"), None);
    }
}
