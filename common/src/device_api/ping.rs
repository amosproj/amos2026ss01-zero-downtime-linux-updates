/// PUT /device/ping - Send aliveness signal
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PutBody {
    pub uptime_secs: i64,
}
