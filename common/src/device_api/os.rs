/// GET /device/os - Get the assigned OS version
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GetResponse {
    pub id: i32,
    pub commit_hash: String,
    pub immediate: bool
}

/// PUT /device/os - Report the currently running OS version
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PutBody {
    pub os_version_id: i32,
}
