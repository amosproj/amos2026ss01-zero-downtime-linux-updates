/// GET /device/apps - Get the assigned applications
pub type GetResponse = Vec<GetResponseItem>;

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GetResponseItem {
    pub id: i32,
    pub application_id: i32,
    pub image: String,
    pub version: i32,
    pub config_version: i32,
    pub config: Option<crate::entities::ContainerConfigV1>,
}

/// PUT /device/apps - Report the currently running applications
pub type PutBody = Vec<PutBodyItem>;

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PutBodyItem {
    pub application_config_id: i32,
}
