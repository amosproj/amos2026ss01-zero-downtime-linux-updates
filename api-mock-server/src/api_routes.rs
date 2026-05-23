use amos_common::entities::{
    Application, ApplicationAssignment, ApplicationConfig, Device, Group, Tenant, OsAssignment, OsVersion,
};
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::json;
use crate::db;

fn err(status: StatusCode, message: impl std::fmt::Display) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

fn not_found(resource: &str, id: i32) -> Response {
    err(StatusCode::NOT_FOUND, format!("{} with id {} not found", resource, id))
}

fn db_err(e: sea_orm::DbErr) -> Response {
    err(StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e))
}

pub fn routes() -> Router {
    Router::new()
        .route("/devices/summary", get(list_device_summaries))
        .route("/devices/:id/summary", get(get_device_summary))
        .route("/tenants", get(list_tenants).post(create_tenant))
        .route("/tenants/:id", get(get_tenant).put(update_tenant).delete(delete_tenant))
        .route("/groups", get(list_groups).post(create_group))
        .route("/groups/:id", get(get_group).put(update_group).delete(delete_group))
        .route("/devices", get(list_devices).post(create_device))
        .route("/devices/:id", get(get_device).put(update_device).delete(delete_device))
        .route("/applications", get(list_applications).post(create_application))
        .route("/applications/:id", get(get_application).put(update_application).delete(delete_application))
        .route("/app-configs", get(list_application_configs).post(create_application_config))
        .route("/app-configs/:id", get(get_application_config).put(update_application_config).delete(delete_application_config))
        .route("/app-assignments", get(list_application_assignments).post(create_application_assignment))
        .route("/app-assignments/:id", get(get_application_assignment).put(update_application_assignment).delete(delete_application_assignment))
        .route("/os-versions", get(list_os_versions).post(create_os_version))
        .route("/os-versions/:id", get(get_os_version).put(update_os_version).delete(delete_os_version))
        .route("/os-assignments", get(list_os_assignments).post(create_os_assignment))
        .route("/os-assignments/:id", get(get_os_assignment).put(update_os_assignment).delete(delete_os_assignment))
}

// --Device Summary--

#[derive(Deserialize)]
struct DeviceQuery {
    group_id: Option<i32>,
    tenant_id: Option<i32>,
}

async fn list_device_summaries(Query(params): Query<DeviceQuery>) -> Response {
    match db::list_device_summaries(params.group_id, params.tenant_id).await {
        Ok(summaries) => Json(summaries).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_device_summary(Path(id): Path<i32>) -> Response {
    match db::get_device_summary(id).await {
        Ok(Some(summary)) => Json(summary).into_response(),
        Ok(None) => not_found("Device", id),
        Err(err) => db_err(err),
    }
}

// --Tenants--

async fn list_tenants() -> Response {
    match db::list_tenants().await {
        Ok(tenants) => Json(tenants).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_tenant(Path(id): Path<i32>) -> Response {
    match db::get_tenant(id).await {
        Ok(Some(tenant)) => Json(tenant).into_response(),
        Ok(None) => not_found("Tenant", id),
        Err(err) => db_err(err),
    }
}

async fn create_tenant(Json(body): Json<Tenant::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Tenant name cannot be empty");
    }
    match db::add_tenant(body.name, body.description).await {
        Ok(tenant) => (StatusCode::CREATED, Json(tenant)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_tenant(Path(id): Path<i32>, Json(body): Json<Tenant::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Tenant name cannot be empty");
    }
    match db::update_tenant(id, body.name, body.description).await {
        Ok(tenant) => Json(tenant).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_tenant(Path(id): Path<i32>) -> Response {
    match db::delete_tenant(id).await {
        Ok(0) => not_found("Tenant", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --Groups--

async fn list_groups() -> Response {
    match db::list_groups().await {
        Ok(groups) => Json(groups).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_group(Path(id): Path<i32>) -> Response {
    match db::get_group(id).await {
        Ok(Some(group)) => Json(group).into_response(),
        Ok(None) => not_found("Group", id),
        Err(err) => db_err(err),
    }
}

async fn create_group(Json(body): Json<Group::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Group name cannot be empty");
    }
    match db::add_group(body.name).await {
        Ok(group) => (StatusCode::CREATED, Json(group)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_group(Path(id): Path<i32>, Json(body): Json<Group::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Group name cannot be empty");
    }
    match db::update_group(id, body.name).await {
        Ok(group) => Json(group).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_group(Path(id): Path<i32>) -> Response {
    match db::delete_group(id).await {
        Ok(0) => not_found("Group", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --Devices--

async fn list_devices(Query(params): Query<DeviceQuery>) -> Response {
    match db::list_devices(params.group_id, params.tenant_id).await {
        Ok(devices) => Json(devices).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_device(Path(id): Path<i32>) -> Response {
    match db::get_device(id).await {
        Ok(Some(device)) => Json(device).into_response(),
        Ok(None) => not_found("Device", id),
        Err(err) => db_err(err),
    }
}

async fn create_device(Json(body): Json<Device::Model>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Device UUID cannot be empty");
    }
    if body.hostname.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Device hostname cannot be empty");
    }
    match db::add_device(body.uuid, body.hostname, body.tenant_id, body.group_id).await {
        Ok(device) => (StatusCode::CREATED, Json(device)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_device(Path(id): Path<i32>, Json(body): Json<Device::Model>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Device UUID cannot be empty");
    }
    if body.hostname.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Device hostname cannot be empty");
    }
    match db::update_device(id, body.uuid, body.hostname, body.tenant_id, body.group_id).await {
        Ok(device) => Json(device).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_device(Path(id): Path<i32>) -> Response {
    match db::delete_device(id).await {
        Ok(0) => not_found("Device", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --Applications--

async fn list_applications() -> Response {
    match db::list_applications().await {
        Ok(applications) => Json(applications).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_application(Path(id): Path<i32>) -> Response {
    match db::get_application(id).await {
        Ok(Some(application)) => Json(application).into_response(),
        Ok(None) => not_found("Application", id),
        Err(err) => db_err(err),
    }
}

async fn create_application(Json(body): Json<Application::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Application name cannot be empty");
    }
    match db::add_application(body.name, body.description).await {
        Ok(application) => (StatusCode::CREATED, Json(application)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_application(Path(id): Path<i32>, Json(body): Json<Application::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Application name cannot be empty");
    }
    match db::update_application(id, body.name, body.description).await {
        Ok(application) => Json(application).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_application(Path(id): Path<i32>) -> Response {
    match db::delete_application(id).await {
        Ok(0) => not_found("Application", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --Application Configs--

#[derive(Deserialize)]
struct AppConfigQuery {
    application_id: Option<i32>,
}

async fn list_application_configs(Query(params): Query<AppConfigQuery>) -> Response {
    match db::list_application_configs(params.application_id).await {
        Ok(configs) => Json(configs).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_application_config(Path(id): Path<i32>) -> Response {
    match db::get_application_config(id).await {
        Ok(Some(config)) => Json(config).into_response(),
        Ok(None) => not_found("ApplicationConfig", id),
        Err(err) => db_err(err),
    }
}

async fn create_application_config(Json(body): Json<ApplicationConfig::Model>) -> Response {
    if body.image.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "ApplicationConfig image cannot be empty");
    }
    match db::add_application_config(body.application_id, body.image, body.config, body.comment).await {
        Ok(config) => (StatusCode::CREATED, Json(config)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_application_config(Path(id): Path<i32>, Json(body): Json<ApplicationConfig::Model>) -> Response {
    if body.image.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "ApplicationConfig image cannot be empty");
    }
    match db::update_application_config(id, body.application_id, body.image, body.config, body.comment).await {
        Ok(config) => Json(config).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_application_config(Path(id): Path<i32>) -> Response {
    match db::delete_application_config(id).await {
        Ok(0) => not_found("ApplicationConfig", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --Application Assignments--

#[derive(Deserialize)]
struct AppAssignmentQuery {
    application_config_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
}

async fn list_application_assignments(Query(params): Query<AppAssignmentQuery>) -> Response {
    match db::list_application_assignments(params.application_config_id, params.device_id, params.group_id).await {
        Ok(assignments) => Json(assignments).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_application_assignment(Path(id): Path<i32>) -> Response {
    match db::get_application_assignment(id).await {
        Ok(Some(assignment)) => Json(assignment).into_response(),
        Ok(None) => not_found("ApplicationAssignment", id),
        Err(err) => db_err(err),
    }
}

async fn create_application_assignment(Json(body): Json<ApplicationAssignment::Model>) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Either device_id or group_id must be set");
    }
    match db::add_application_assignment(body.application_config_id, body.device_id, body.group_id).await {
        Ok(assignment) => (StatusCode::CREATED, Json(assignment)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_application_assignment(Path(id): Path<i32>, Json(body): Json<ApplicationAssignment::Model>) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Either device_id or group_id must be set");
    }
    match db::update_application_assignment(id, body.application_config_id, body.device_id, body.group_id).await {
        Ok(assignment) => Json(assignment).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_application_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_application_assignment(id).await {
        Ok(0) => not_found("ApplicationAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --OS Versions--

async fn list_os_versions() -> Response {
    match db::list_os_versions().await {
        Ok(os_versions) => Json(os_versions).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_os_version(Path(id): Path<i32>) -> Response {
    match db::get_os_version(id).await {
        Ok(Some(os_version)) => Json(os_version).into_response(),
        Ok(None) => not_found("OsVersion", id),
        Err(err) => db_err(err),
    }
}

async fn create_os_version(Json(body): Json<OsVersion::Model>) -> Response {
    if body.commit_hash.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "OS Version commit hash cannot be empty");
    }
    if body.orchestrator_version.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "OS Version orchestrator version cannot be empty");
    }
    match db::add_os_version(body.commit_hash, body.orchestrator_version, body.description).await {
        Ok(os_version) => (StatusCode::CREATED, Json(os_version)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_os_version(Path(id): Path<i32>, Json(body): Json<OsVersion::Model>) -> Response {
    if body.commit_hash.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "OS Version commit hash cannot be empty");
    }
    if body.orchestrator_version.trim().is_empty() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "OS Version orchestrator version cannot be empty");
    }
    match db::update_os_version(id, body.commit_hash, body.orchestrator_version, body.description).await {
        Ok(os_version) => Json(os_version).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_os_version(Path(id): Path<i32>) -> Response {
    match db::delete_os_version(id).await {
        Ok(0) => not_found("OsVersion", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --OS Assignments--

#[derive(Deserialize)]
struct OsAssignmentQuery {
    os_version_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
}

async fn list_os_assignments(Query(params): Query<OsAssignmentQuery>) -> Response {
    match db::list_os_assignments(params.os_version_id, params.device_id, params.group_id).await {
        Ok(assignments) => Json(assignments).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_os_assignment(Path(id): Path<i32>) -> Response {
    match db::get_os_assignment(id).await {
        Ok(Some(assignment)) => Json(assignment).into_response(),
        Ok(None) => not_found("OsAssignment", id),
        Err(err) => db_err(err),
    }
}

async fn create_os_assignment(Json(body): Json<OsAssignment::Model>) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Either device_id or group_id must be set");
    }
    match db::add_os_assignment(body.os_version_id, body.device_id, body.group_id).await {
        Ok(assignment) => (StatusCode::CREATED, Json(assignment)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_os_assignment(Path(id): Path<i32>, Json(body): Json<OsAssignment::Model>) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Either device_id or group_id must be set");
    }
    match db::update_os_assignment(id, body.os_version_id, body.device_id, body.group_id).await {
        Ok(assignment) => Json(assignment).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_os_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_os_assignment(id).await {
        Ok(0) => not_found("OsAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}
