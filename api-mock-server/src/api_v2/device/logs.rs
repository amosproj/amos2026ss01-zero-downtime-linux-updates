use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};

use crate::{api_v2::db::DataStore, auth::extractors::AuthDevice};

/// POST /device/logs - Publish some log lines
pub async fn post(
    State(db): State<DataStore>,
    AuthDevice(device): AuthDevice,
    Query(params): Query<amos_common::device_api::logs::PostQueryParams>,
    Json(body): Json<amos_common::device_api::logs::PostBody>,
) -> StatusCode {
    StatusCode::OK
}
