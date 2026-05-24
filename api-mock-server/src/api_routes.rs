use crate::db;
use amos_common::entities::{
    Application, ApplicationAssignment, ApplicationConfig, Device, Group, OsAssignment, OsVersion,
    ReportedApplicationAssignment, ReportedOsAssignment, Tenant,
};
use axum::{
    Json, Router,
    extract::{Path, Query},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::json;

fn err(status: StatusCode, message: impl std::fmt::Display) -> Response {
    (status, Json(json!({ "error": message.to_string() }))).into_response()
}

fn not_found(resource: &str, id: i32) -> Response {
    err(
        StatusCode::NOT_FOUND,
        format!("{} with id {} not found", resource, id),
    )
}

fn db_err(e: sea_orm::DbErr) -> Response {
    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Database error: {}", e),
    )
}

/// # API Routes
///
/// All routes are mounted under `/v1` (see `main.rs`).
///
/// ## Catalog (static)
/// - `GET  /v1/catalog`                        — Returns the static catalog of desired OS and app versions.
///
/// ## Device Summaries (read-only)
/// - `GET  /v1/devices/summary`                — List device summaries. Optional query: `?tenant_id=<i32>`
/// - `GET  /v1/devices/{id}/summary`           — Get a single device summary by ID.
///
/// ## Tenants
/// - `GET    /v1/tenants`                      — List all tenants.
/// - `POST   /v1/tenants`                      — Create a tenant. Body: `{ name: string (required), description: string|null }`
/// - `GET    /v1/tenants/{id}`                 — Get a tenant by ID.
/// - `PUT    /v1/tenants/{id}`                 — Replace a tenant by ID.
/// - `DELETE /v1/tenants/{id}`                 — Delete a tenant by ID. Returns 204 on success.
///
/// ## Groups
/// - `GET    /v1/groups`                       — List all groups.
/// - `POST   /v1/groups`                       — Create a group. Body: `{ name: string (required) }`
/// - `GET    /v1/groups/{id}`                  — Get a group by ID.
/// - `PUT    /v1/groups/{id}`                  — Replace a group by ID.
/// - `DELETE /v1/groups/{id}`                  — Delete a group by ID. Returns 204 on success.
///
/// ## Devices
/// - `GET    /v1/devices`                      — List devices. Optional query: `?group_id=<i32>&tenant_id=<i32>`
/// - `POST   /v1/devices`                      — Create a device. Body: `{ uuid: string (required), hostname: string (required), tenant_id: i32|null, group_id: i32|null }`
/// - `GET    /v1/devices/{id}`                 — Get a device by ID.
/// - `PUT    /v1/devices/{id}`                 — Replace a device by ID.
/// - `DELETE /v1/devices/{id}`                 — Delete a device by ID. Returns 204 on success.
///
/// ## Applications
/// - `GET    /v1/applications`                 — List all applications.
/// - `POST   /v1/applications`                 — Create an application. Body: `{ name: string (required), description: string|null }`
/// - `GET    /v1/applications/{id}`            — Get an application by ID.
/// - `PUT    /v1/applications/{id}`            — Replace an application by ID.
/// - `DELETE /v1/applications/{id}`            — Delete an application by ID. Returns 204 on success.
///
/// ## Application Configs
/// - `GET    /v1/app-configs`                  — List app configs. Optional query: `?application_id=<i32>`
/// - `POST   /v1/app-configs`                  — Create an app config. Body: `{ application_id: i32, image: string (required), config: string|null, comment: string|null }`
/// - `GET    /v1/app-configs/{id}`             — Get an app config by ID.
/// - `PUT    /v1/app-configs/{id}`             — Replace an app config by ID.
/// - `DELETE /v1/app-configs/{id}`             — Delete an app config by ID. Returns 204 on success.
///
/// ## Application Assignments
/// - `GET    /v1/app-assignments`              — List app assignments. Optional query: `?application_config_id=<i32>&device_id=<i32>&group_id=<i32>`
/// - `POST   /v1/app-assignments`             — Create an app assignment. Body: `{ application_config_id: i32, device_id: i32|null, group_id: i32|null }` — at least one of `device_id`/`group_id` required.
/// - `GET    /v1/app-assignments/{id}`         — Get an app assignment by ID.
/// - `PUT    /v1/app-assignments/{id}`         — Replace an app assignment by ID.
/// - `DELETE /v1/app-assignments/{id}`         — Delete an app assignment by ID. Returns 204 on success.
///
/// ## Reported Application Assignments (device-originated, no POST/PUT)
/// - `GET    /v1/reported-app-assignments`     — List reported app assignments. Optional query: `?application_config_id=<i32>&device_id=<i32>`
/// - `GET    /v1/reported-app-assignments/{id}` — Get a reported app assignment by ID.
/// - `DELETE /v1/reported-app-assignments/{id}` — Delete a reported app assignment by ID. Returns 204 on success.
///
/// ## OS Versions
/// - `GET    /v1/os-versions`                  — List all OS versions.
/// - `POST   /v1/os-versions`                  — Create an OS version. Body: `{ commit_hash: string (required), orchestrator_version: string (required), description: string|null }`
/// - `GET    /v1/os-versions/{id}`             — Get an OS version by ID.
/// - `PUT    /v1/os-versions/{id}`             — Replace an OS version by ID.
/// - `DELETE /v1/os-versions/{id}`             — Delete an OS version by ID. Returns 204 on success.
///
/// ## OS Assignments
/// - `GET    /v1/os-assignments`               — List OS assignments. Optional query: `?os_version_id=<i32>&device_id=<i32>&group_id=<i32>`
/// - `POST   /v1/os-assignments`              — Create an OS assignment. Body: `{ os_version_id: i32, device_id: i32|null, group_id: i32|null }` — at least one of `device_id`/`group_id` required.
/// - `GET    /v1/os-assignments/{id}`          — Get an OS assignment by ID.
/// - `PUT    /v1/os-assignments/{id}`          — Replace an OS assignment by ID.
/// - `DELETE /v1/os-assignments/{id}`          — Delete an OS assignment by ID. Returns 204 on success.
///
/// ## Reported OS Assignments (device-originated, no POST/PUT)
/// - `GET    /v1/reported-os-assignments`      — List reported OS assignments. Optional query: `?os_version_id=<i32>&device_id=<i32>`
/// - `GET    /v1/reported-os-assignments/{id}` — Get a reported OS assignment by ID.
/// - `DELETE /v1/reported-os-assignments/{id}` — Delete a reported OS assignment by ID. Returns 204 on success.
///
/// ## Error responses
/// All errors return JSON: `{ "error": "<message>" }` with an appropriate HTTP status code.
/// - 404 Not Found — resource with that ID does not exist.
/// - 422 Unprocessable Entity — validation failed (e.g. empty required field, missing device_id/group_id).
/// - 500 Internal Server Error — database error.
pub fn routes() -> Router {
    Router::new()
        .route("/devices/summary", get(list_device_summaries))
        .route("/devices/{id}/summary", get(get_device_summary))
        .route("/tenants", get(list_tenants).post(create_tenant))
        .route(
            "/tenants/{id}",
            get(get_tenant).put(update_tenant).delete(delete_tenant),
        )
        .route("/groups", get(list_groups).post(create_group))
        .route(
            "/groups/{id}",
            get(get_group).put(update_group).delete(delete_group),
        )
        .route("/devices", get(list_devices).post(create_device))
        .route(
            "/devices/{id}",
            get(get_device).put(update_device).delete(delete_device),
        )
        .route(
            "/applications",
            get(list_applications).post(create_application),
        )
        .route(
            "/applications/{id}",
            get(get_application)
                .put(update_application)
                .delete(delete_application),
        )
        .route(
            "/app-configs",
            get(list_application_configs).post(create_application_config),
        )
        .route(
            "/app-configs/{id}",
            get(get_application_config)
                .put(update_application_config)
                .delete(delete_application_config),
        )
        .route(
            "/app-assignments",
            get(list_application_assignments).post(create_application_assignment),
        )
        .route(
            "/app-assignments/{id}",
            get(get_application_assignment)
                .put(update_application_assignment)
                .delete(delete_application_assignment),
        )
        .route(
            "/reported-app-assignments", // No POST/PUT endpoint for reported assignments, they should come from devices
            get(list_reported_application_assignments),
        )
        .route(
            "/reported-app-assignments/{id}",
            get(get_reported_application_assignment).delete(delete_reported_application_assignment),
        )
        .route(
            "/os-versions",
            get(list_os_versions).post(create_os_version),
        )
        .route(
            "/os-versions/{id}",
            get(get_os_version)
                .put(update_os_version)
                .delete(delete_os_version),
        )
        .route(
            "/os-assignments",
            get(list_os_assignments).post(create_os_assignment),
        )
        .route(
            "/os-assignments/{id}",
            get(get_os_assignment)
                .put(update_os_assignment)
                .delete(delete_os_assignment),
        )
        .route(
            "/reported-os-assignments", // No POST/PUT endpoint for reported assignments, they should come from devices
            get(list_reported_os_assignments),
        )
        .route(
            "/reported-os-assignments/{id}",
            get(get_reported_os_assignment).delete(delete_reported_os_assignment),
        )
}

// --Device Summary--

#[derive(Deserialize)]
struct DeviceQuery {
    group_id: Option<i32>,
    tenant_id: Option<i32>,
}

async fn list_device_summaries(Query(params): Query<DeviceQuery>) -> Response {
    match db::list_device_summaries(params.tenant_id).await {
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Tenant name cannot be empty",
        );
    }
    match db::add_tenant(body.name, body.description).await {
        Ok(tenant) => (StatusCode::CREATED, Json(tenant)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_tenant(Path(id): Path<i32>, Json(body): Json<Tenant::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Tenant name cannot be empty",
        );
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Group name cannot be empty",
        );
    }
    match db::add_group(body.name).await {
        Ok(group) => (StatusCode::CREATED, Json(group)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_group(Path(id): Path<i32>, Json(body): Json<Group::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Group name cannot be empty",
        );
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device UUID cannot be empty",
        );
    }
    if body.hostname.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device hostname cannot be empty",
        );
    }
    match db::add_device(body.uuid, body.hostname, body.tenant_id, body.group_id).await {
        Ok(device) => (StatusCode::CREATED, Json(device)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_device(Path(id): Path<i32>, Json(body): Json<Device::Model>) -> Response {
    if body.uuid.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device UUID cannot be empty",
        );
    }
    if body.hostname.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Device hostname cannot be empty",
        );
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Application name cannot be empty",
        );
    }
    match db::add_application(body.name, body.description).await {
        Ok(application) => (StatusCode::CREATED, Json(application)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_application(Path(id): Path<i32>, Json(body): Json<Application::Model>) -> Response {
    if body.name.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Application name cannot be empty",
        );
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ApplicationConfig image cannot be empty",
        );
    }
    match db::add_application_config(body.application_id, body.image, body.config, body.comment)
        .await
    {
        Ok(config) => (StatusCode::CREATED, Json(config)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_application_config(
    Path(id): Path<i32>,
    Json(body): Json<ApplicationConfig::Model>,
) -> Response {
    if body.image.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "ApplicationConfig image cannot be empty",
        );
    }
    match db::update_application_config(
        id,
        body.application_id,
        body.image,
        body.config,
        body.comment,
    )
    .await
    {
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
    match db::list_application_assignments(
        params.application_config_id,
        params.device_id,
        params.group_id,
    )
    .await
    {
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        );
    }
    match db::add_application_assignment(body.application_config_id, body.device_id, body.group_id)
        .await
    {
        Ok(assignment) => (StatusCode::CREATED, Json(assignment)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_application_assignment(
    Path(id): Path<i32>,
    Json(body): Json<ApplicationAssignment::Model>,
) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        );
    }
    match db::update_application_assignment(
        id,
        body.application_config_id,
        body.device_id,
        body.group_id,
    )
    .await
    {
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

// --Reported Application Assignments--

#[derive(Deserialize)]
struct ReportedAppAssignmentQuery {
    application_config_id: Option<i32>,
    device_id: Option<i32>,
}

async fn list_reported_application_assignments(
    Query(params): Query<ReportedAppAssignmentQuery>,
) -> Response {
    match db::list_reported_application_assignments(params.application_config_id, params.device_id)
        .await
    {
        Ok(assignments) => Json(assignments).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_reported_application_assignment(Path(id): Path<i32>) -> Response {
    match db::get_reported_application_assignment(id).await {
        Ok(Some(assignment)) => Json(assignment).into_response(),
        Ok(None) => not_found("ReportedApplicationAssignment", id),
        Err(err) => db_err(err),
    }
}

#[allow(dead_code)]
async fn create_reported_application_assignment(
    Json(body): Json<ReportedApplicationAssignment::Model>,
) -> Response {
    match db::add_reported_application_assignment(body.application_config_id, body.device_id).await
    {
        Ok(assignment) => (StatusCode::CREATED, Json(assignment)).into_response(),
        Err(err) => db_err(err),
    }
}

#[allow(dead_code)]
async fn update_reported_application_assignment(
    Path(id): Path<i32>,
    Json(body): Json<ReportedApplicationAssignment::Model>,
) -> Response {
    match db::update_reported_application_assignment(id, body.application_config_id, body.device_id)
        .await
    {
        Ok(assignment) => Json(assignment).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_reported_application_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_reported_application_assignment(id).await {
        Ok(0) => not_found("ReportedApplicationAssignment", id),
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version commit hash cannot be empty",
        );
    }
    if body.orchestrator_version.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version orchestrator version cannot be empty",
        );
    }
    match db::add_os_version(
        body.commit_hash,
        body.orchestrator_version,
        body.description,
    )
    .await
    {
        Ok(os_version) => (StatusCode::CREATED, Json(os_version)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_os_version(Path(id): Path<i32>, Json(body): Json<OsVersion::Model>) -> Response {
    if body.commit_hash.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version commit hash cannot be empty",
        );
    }
    if body.orchestrator_version.trim().is_empty() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "OS Version orchestrator version cannot be empty",
        );
    }
    match db::update_os_version(
        id,
        body.commit_hash,
        body.orchestrator_version,
        body.description,
    )
    .await
    {
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
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        );
    }
    match db::add_os_assignment(body.os_version_id, body.device_id, body.group_id).await {
        Ok(assignment) => (StatusCode::CREATED, Json(assignment)).into_response(),
        Err(err) => db_err(err),
    }
}

async fn update_os_assignment(
    Path(id): Path<i32>,
    Json(body): Json<OsAssignment::Model>,
) -> Response {
    if body.device_id.is_none() && body.group_id.is_none() {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Either device_id or group_id must be set",
        );
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

// --Reported OS Assignments--

#[derive(Deserialize)]
struct ReportedOsAssignmentQuery {
    os_version_id: Option<i32>,
    device_id: Option<i32>,
}

async fn list_reported_os_assignments(Query(params): Query<ReportedOsAssignmentQuery>) -> Response {
    match db::list_reported_os_assignments(params.device_id, params.os_version_id).await {
        Ok(assignments) => Json(assignments).into_response(),
        Err(err) => db_err(err),
    }
}

async fn get_reported_os_assignment(Path(id): Path<i32>) -> Response {
    match db::get_reported_os_assignment(id).await {
        Ok(Some(assignment)) => Json(assignment).into_response(),
        Ok(None) => not_found("ReportedOsAssignment", id),
        Err(err) => db_err(err),
    }
}

#[allow(dead_code)]
async fn create_reported_os_assignment(Json(body): Json<ReportedOsAssignment::Model>) -> Response {
    match db::add_reported_os_assignment(body.os_version_id, body.device_id).await {
        Ok(assignment) => (StatusCode::CREATED, Json(assignment)).into_response(),
        Err(err) => db_err(err),
    }
}

#[allow(dead_code)]
async fn update_reported_os_assignment(
    Path(id): Path<i32>,
    Json(body): Json<ReportedOsAssignment::Model>,
) -> Response {
    match db::update_reported_os_assignment(id, body.os_version_id, body.device_id).await {
        Ok(assignment) => Json(assignment).into_response(),
        Err(err) => db_err(err),
    }
}

async fn delete_reported_os_assignment(Path(id): Path<i32>) -> Response {
    match db::delete_reported_os_assignment(id).await {
        Ok(0) => not_found("ReportedOsAssignment", id),
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => db_err(err),
    }
}

// --Tests--

#[cfg(test)]
mod tests {
    use super::routes;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
    };
    use serial_test::serial;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        crate::db::initialialize_db("sqlite::memory:".into())
            .await
            .unwrap();
        Router::new().nest("/v1", routes())
    }

    async fn get(app: Router, uri: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn post(app: Router, uri: &str, json: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header("content-type", "application/json")
                    .body(Body::from(json.to_owned()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn delete(app: Router, uri: &str) -> StatusCode {
        app.oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .status()
    }

    // --- Tenants ---
    // Tenant::Model: { id: i32, name: String, description: Option<String> }

    #[tokio::test]
    #[serial]
    async fn test_list_tenants_returns_200_and_empty_array() {
        let (status, body) = get(test_app().await, "/v1/tenants").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_tenant_returns_201_with_created_entity() {
        let (status, body) = post(
            test_app().await,
            "/v1/tenants",
            // description is Option<String> so null is valid
            r#"{"id":0,"name":"Acme","description":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["name"], "Acme");
        assert_eq!(json["description"], serde_json::Value::Null);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_tenant_with_description_returns_201() {
        let (status, body) = post(
            test_app().await,
            "/v1/tenants",
            r#"{"id":0,"name":"Acme","description":"A real company"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["description"], "A real company");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_tenant_with_empty_name_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/tenants",
            r#"{"id":0,"name":"","description":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_tenant_not_found_returns_404() {
        let (status, body) = get(test_app().await, "/v1/tenants/999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json["error"].as_str().is_some());
    }

    #[tokio::test]
    #[serial]
    async fn test_delete_tenant_not_found_returns_404() {
        assert_eq!(
            delete(test_app().await, "/v1/tenants/999").await,
            StatusCode::NOT_FOUND
        );
    }

    // --- Groups ---
    // Group::Model: { id: i32, name: String }

    #[tokio::test]
    #[serial]
    async fn test_list_groups_returns_200_and_empty_array() {
        let (status, body) = get(test_app().await, "/v1/groups").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_group_returns_201() {
        let (status, body) = post(
            test_app().await,
            "/v1/groups",
            r#"{"id":0,"name":"Werk Erlangen"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["name"], "Werk Erlangen");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_group_with_empty_name_returns_422() {
        let (status, _) = post(test_app().await, "/v1/groups", r#"{"id":0,"name":""}"#).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // --- Devices ---
    // Device::Model: { id: i32, uuid: String, hostname: String, tenant_id: i32, group_id: Option<i32> }
    // tenant_id is required (non-optional) — must always be present in POST body.
    // A non-existent tenant_id will produce a 500 (FK violation), not 422.

    #[tokio::test]
    #[serial]
    async fn test_list_devices_returns_200_and_empty_array() {
        let (status, body) = get(test_app().await, "/v1/devices").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_device_with_empty_uuid_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/devices",
            r#"{"id":0,"uuid":"","hostname":"host-1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_device_with_empty_hostname_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/devices",
            r#"{"id":0,"uuid":"some-uuid","hostname":"","tenant_id":1,"group_id":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // --- Device summaries ---

    #[tokio::test]
    #[serial]
    async fn test_list_device_summaries_returns_200() {
        let (status, _) = get(test_app().await, "/v1/devices/summary").await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_device_summary_not_found_returns_404() {
        let (status, _) = get(test_app().await, "/v1/devices/999/summary").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // --- Applications ---
    // Application::Model: { id: i32, name: String, description: String }
    // description is NOT optional — must be a non-null string in the POST body.

    #[tokio::test]
    #[serial]
    async fn test_list_applications_returns_200_and_empty_array() {
        let (status, body) = get(test_app().await, "/v1/applications").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_application_returns_201() {
        let (status, body) = post(
            test_app().await,
            "/v1/applications",
            // description is String (required, not nullable)
            r#"{"id":0,"name":"my-app","description":"does things"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["name"], "my-app");
        assert_eq!(json["description"], "does things");
    }

    #[tokio::test]
    #[serial]
    async fn test_create_application_with_empty_name_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/applications",
            r#"{"id":0,"name":"","description":"desc"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // --- Application Configs ---
    // ApplicationConfig::Model: { id: i32, application_id: i32, image: String, config: Option<String>, comment: Option<String> }

    #[tokio::test]
    #[serial]
    async fn test_create_app_config_with_empty_image_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/app-configs",
            r#"{"id":0,"application_id":1,"image":"","config":null,"comment":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // --- OS Versions ---
    // OsVersion::Model: { id: i32, commit_hash: String, orchestrator_version: String, description: Option<String> }
    // description IS optional here (unlike Application).

    #[tokio::test]
    #[serial]
    async fn test_create_os_version_with_empty_commit_hash_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"","orchestrator_version":"1.0","description":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_os_version_with_empty_orchestrator_version_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"abc123","orchestrator_version":"","description":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_os_version_with_null_description_returns_201() {
        let (status, body) = post(
            test_app().await,
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"abc123","orchestrator_version":"1.0","description":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["description"], serde_json::Value::Null);
    }

    // --- OS / App Assignments ---
    // OsAssignment::Model:  { os_version_id: i32, device_id: Option<i32>, group_id: Option<i32> }
    // ApplicationAssignment::Model: { application_config_id: i32, device_id: Option<i32>, group_id: Option<i32> }
    // Both enforce device_id OR group_id in the handler (422) AND in before_save (500 backstop).

    #[tokio::test]
    #[serial]
    async fn test_create_os_assignment_without_device_or_group_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/os-assignments",
            r#"{"id":0,"os_version_id":1,"device_id":null,"group_id":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_app_assignment_without_device_or_group_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/app-assignments",
            r#"{"id":0,"application_config_id":1,"device_id":null,"group_id":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // --- Reported assignments ---
    // ReportedApplicationAssignment::Model: { id: i32, application_config_id: i32, device_id: i32, updated_at: DateTimeUtc }
    // ReportedOsAssignment::Model:          { id: i32, os_version_id: i32, device_id: i32, updated_at: DateTimeUtc }
    // device_id is NOT optional on reported assignments (i32, not Option<i32>).
    // No POST/PUT routes are registered for these — only GET and DELETE.

    #[tokio::test]
    #[serial]
    async fn test_post_reported_app_assignments_returns_405() {
        let (status, _) = post(
            test_app().await,
            "/v1/reported-app-assignments",
            r#"{"id":0,"application_config_id":1,"device_id":1,"updated_at":"2024-01-01T00:00:00Z"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    #[serial]
    async fn test_post_reported_os_assignments_returns_405() {
        let (status, _) = post(
            test_app().await,
            "/v1/reported-os-assignments",
            r#"{"id":0,"os_version_id":1,"device_id":1,"updated_at":"2024-01-01T00:00:00Z"}"#,
        )
        .await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_reported_app_assignments_returns_200() {
        let (status, body) = get(test_app().await, "/v1/reported-app-assignments").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    #[tokio::test]
    #[serial]
    async fn test_list_reported_os_assignments_returns_200() {
        let (status, body) = get(test_app().await, "/v1/reported-os-assignments").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "[]");
    }

    // --- Error response shape ---

    #[tokio::test]
    #[serial]
    async fn test_not_found_response_contains_error_field() {
        let (_, body) = get(test_app().await, "/v1/tenants/42").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(
            json.get("error").is_some(),
            "expected an 'error' field in 404 response, got: {body}"
        );
    }
}
