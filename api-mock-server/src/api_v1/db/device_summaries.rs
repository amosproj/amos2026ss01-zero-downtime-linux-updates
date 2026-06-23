use crate::dtos;
use sea_orm::DbErr;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, EntityTrait, ExprTrait, PaginatorTrait, QueryFilter, QueryOrder};
use serde_json::json;

use super::db;

// --Device Summary--

pub async fn get_device_summary(id: i32) -> Result<Option<serde_json::Value>, DbErr> {
    let mut res = assemble_device_summary(Some(id), None, None, None, None, 0, 1).await?;
    Ok(res.0.pop())
}

pub async fn list_device_summaries(
    group_id: Option<i32>,
    tenant_id: Option<i32>,
    uuid_filter: Option<String>,
    hostname_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<serde_json::Value>, u64), DbErr> {
    assemble_device_summary(
        None,
        group_id,
        tenant_id,
        uuid_filter,
        hostname_filter,
        page,
        page_size,
    )
    .await
}

pub async fn assemble_device_summary(
    device_id: Option<i32>,
    group_id: Option<i32>,
    tenant_id: Option<i32>,
    uuid_filter: Option<String>,
    hostname_filter: Option<String>,
    page: u64,
    page_size: u64,
) -> Result<(Vec<serde_json::Value>, u64), DbErr> {
    let db = db!();

    let mut query = dtos::Device::Entity::find().order_by_asc(dtos::Device::Column::Id);
    if let Some(id) = device_id {
        query = query.filter(dtos::Device::Column::Id.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(dtos::Device::Column::GroupId.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(dtos::Device::Column::TenantId.eq(id));
    }
    if let Some(uuid) = uuid_filter {
        query = query.filter(Expr::col(dtos::Device::Column::Uuid).like(format!("%{}%", uuid)));
    }
    if let Some(hostname) = hostname_filter {
        query =
            query.filter(Expr::col(dtos::Device::Column::Hostname).like(format!("%{}%", hostname)));
    }

    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let devices = paginator.fetch_page(page).await?;

    if devices.is_empty() {
        return Ok((vec![], total_items));
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
                "config": config.as_ref().map(|c| &c.config),
                "image": config.as_ref().map(|c| &c.image),
                "version": config.as_ref().map(|c| c.version),
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

    Ok((summaries, total_items))
}
