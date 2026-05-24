use axum::{extract::State, http::StatusCode, Json};
use sea_orm::{entity::prelude::*, ActiveValue::Set, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::db::os_version;
use crate::db::reported_os_assignment;

#[derive(Deserialize)]
pub struct DeviceSyncRequest {
    pub device_uuid: String,
    pub current_os_version: String,
}

#[derive(Serialize)]
pub struct CloudSyncResponse {
    pub target_os_commit_hash: Option<String>,
    pub orchestrator_version: Option<String>,
    pub description: Option<String>,
}

// Need to rename this
pub async fn handle_device_sync_here(
    State(db): State<DatabaseConnection>,
    Json(payload): Json<DeviceSyncRequest>,
) -> Result<Json<CloudSyncResponse>, (StatusCode, String)> {
    
    /*
    let device_model = device::Entity::find()
        .filter(device::Column::Uuid.eq(&payload.device_uuid))
        .one(&db).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Device not found".to_string()))?;

    let current_os_model = os_version::Entity::find()
        .filter(os_version::Column::CommitHash.eq(&payload.current_os_version))
        .one(&db).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "OS Version not found".to_string()))?;
    */
    
    let device_id = 1; // Placeholder for device_model.id
    let current_os_id = 1; // Placeholder for current_os_model.id

    let existing_assignment = reported_os_assignment::Entity::find()
        .filter(reported_os_assignment::Column::DeviceId.eq(device_id))
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    match existing_assignment {
        Some(assignment) => {
            let mut active_assignment: reported_os_assignment::ActiveModel = assignment.into();
            active_assignment.os_version_id = Set(current_os_id);
            active_assignment.update(&db).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        None => {
            let new_assignment = reported_os_assignment::ActiveModel {
                id: NotSet, // ?
                device_id: Set(device_id),
                os_version_id: Set(current_os_id),
                updated_at: NotSet, // before_save handles it
            };
            new_assignment.insert(&db).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
    };

    // Need to take custom for rebase isntead of last
    let target_os = os_version::Entity::find()
        .order_by_desc(os_version::Column::Id)
        .one(&db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "No OS versions available".to_string()))?;

    Ok(Json(CloudSyncResponse {
        target_os_commit_hash: Some(target_os.commit_hash),
        orchestrator_version: Some(target_os.orchestrator_version),
        description: target_os.description,
    }))
}