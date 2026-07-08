/// POST /device/logs - Publish some log lines
#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PostQueryParams {
    pub application_id: Option<i32>,
}

pub type PostBody = Vec<PostBodyItem>;

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PostBodyItem {
    pub time: Option<chrono::DateTime<chrono::Utc>>,
    pub level: crate::entities::LogLevel,
    pub message: String,
    pub source: Option<String>,
}
