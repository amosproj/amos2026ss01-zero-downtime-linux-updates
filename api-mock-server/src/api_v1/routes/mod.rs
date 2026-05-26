pub mod application_assignments;
pub mod application_configs;
pub mod applications;
pub mod devices;
pub mod groups;
pub mod os_assignments;
pub mod os_versions;
pub mod reported_application_assignments;
pub mod reported_os_assignments;
pub mod tenants;

use axum::Router;

use amos_common::ErrorResponse;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(super) fn err(status: StatusCode, message: impl ToString) -> Response {
    (
        status,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
        .into_response()
}

pub(super) fn not_found(resource: &str, id: i32) -> Response {
    err(
        StatusCode::NOT_FOUND,
        format!("{} with id {} not found", resource, id),
    )
}

pub(super) fn db_err(e: sea_orm::DbErr) -> Response {
    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Database error: {}", e),
    )
}

pub fn routes() -> Router {
    Router::new()
        .merge(application_assignments::routes())
        .merge(application_configs::routes())
        .merge(applications::routes())
        .merge(devices::routes())
        .merge(groups::routes())
        .merge(os_assignments::routes())
        .merge(os_versions::routes())
        .merge(reported_application_assignments::routes())
        .merge(reported_os_assignments::routes())
        .merge(tenants::routes())
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
        crate::api_v1::db::initialialize_db("sqlite::memory:".into())
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
