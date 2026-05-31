use crate::dtos;
use sea_orm::DbErr;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;

use super::db;

// --Device Summary--

pub async fn get_device_summary(id: i32) -> Result<Option<serde_json::Value>, DbErr> {
    let mut res = assemble_device_summary(Some(id), None).await?;
    Ok(res.pop())
}

pub async fn list_device_summaries(
    tenant_id: Option<i32>,
) -> Result<Vec<serde_json::Value>, DbErr> {
    assemble_device_summary(None, tenant_id).await
}

pub async fn assemble_device_summary(
    device_id: Option<i32>,
    tenant_id: Option<i32>,
) -> Result<Vec<serde_json::Value>, DbErr> {
    let db = db!();

    let mut query = dtos::Device::Entity::find();
    if let Some(id) = device_id {
        query = query.filter(dtos::Device::Column::Id.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(dtos::Device::Column::TenantId.eq(id));
    }

    let devices = query.all(&db).await?;
    if devices.is_empty() {
        return Ok(vec![]);
    }

    let device_ids: Vec<i32> = devices.iter().map(|d| d.id).collect();

    let os_rows = dtos::ReportedOsAssignment::Entity::find()
        .filter(dtos::ReportedOsAssignment::Column::DeviceId.is_in(device_ids.clone()))
        .find_also_related(dtos::OsVersion::Entity)
        .all(&db)
        .await?;

    let mut os_by_device: std::collections::HashMap<i32, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    for (assignment, version) in os_rows {
        if let Some(os_v) = version {
            os_by_device
                .entry(assignment.device_id)
                .or_default()
                .push(json!({
                    "reported_assignment_id": assignment.id,
                    "updated_at": assignment.updated_at,
                    "commit_hash": os_v.commit_hash,
                    "orchestrator_version": os_v.orchestrator_version,
                    "description": os_v.description,
                }));
        }
    }

    let app_rows = dtos::ReportedApplicationAssignment::Entity::find()
        .filter(dtos::ReportedApplicationAssignment::Column::DeviceId.is_in(device_ids.clone()))
        .find_also_related(dtos::ApplicationConfig::Entity)
        .all(&db)
        .await?;

    let app_ids: Vec<i32> = app_rows
        .iter()
        .filter_map(|(_, config)| config.as_ref().map(|c| c.application_id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let app_by_id: std::collections::HashMap<i32, dtos::Application::Model> =
        dtos::Application::Entity::find()
            .filter(dtos::Application::Column::Id.is_in(app_ids))
            .all(&db)
            .await?
            .into_iter()
            .map(|app| (app.id, app))
            .collect();

    let mut apps_by_device: std::collections::HashMap<i32, Vec<serde_json::Value>> =
        std::collections::HashMap::new();

    for (assignment, config) in app_rows {
        let application = config
            .as_ref()
            .and_then(|c| app_by_id.get(&c.application_id));
        apps_by_device
            .entry(assignment.device_id)
            .or_default()
            .push(json!({
                "reported_assignment_id": assignment.id,
                "updated_at": assignment.updated_at,
                "application_name": application.map(|a| a.name.as_str()),
                "application_description": application.map(|a| a.description.as_str()),
                "image": config.as_ref().map(|c| &c.image),
                "config": config.as_ref().and_then(|c| c.config.as_deref()),
                "comment": config.as_ref().and_then(|c| c.comment.as_deref()),
            }))
    }

    let tenant_ids: Vec<i32> = devices
        .iter()
        .map(|d| d.tenant_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let tenant_by_id: std::collections::HashMap<i32, dtos::Tenant::Model> =
        dtos::Tenant::Entity::find()
            .filter(dtos::Tenant::Column::Id.is_in(tenant_ids))
            .all(&db)
            .await?
            .into_iter()
            .map(|tenant| (tenant.id, tenant))
            .collect();

    let summaries = devices
        .into_iter()
        .map(|device| {
            let os = os_by_device.remove(&device.id).unwrap_or_default();
            let apps = apps_by_device.remove(&device.id).unwrap_or_default();
            let tenant = tenant_by_id.get(&device.tenant_id);
            json!({
                "device": device.into_api(),
                "tenant": tenant.map(|t| t.clone().into_api()),
                "os_versions": os,
                "applications": apps,
            })
        })
        .collect();

    Ok(summaries)
}
