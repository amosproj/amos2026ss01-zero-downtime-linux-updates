use std::sync::OnceLock;

use amos_common::entities::LogEvent;
use tokio::sync::broadcast;

static LOG_EVENTS: OnceLock<broadcast::Sender<LogEvent>> = OnceLock::new();

pub fn sender() -> broadcast::Sender<LogEvent> {
    LOG_EVENTS
        .get_or_init(|| broadcast::channel(1024).0)
        .clone()
}

pub fn publish(event: LogEvent) {
    let _ = sender().send(event);
}
