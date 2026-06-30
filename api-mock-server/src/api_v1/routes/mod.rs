pub mod application_assignments;
pub mod application_configs;
pub mod applications;
pub mod audit_log;
pub mod devices;
pub mod groups;
pub mod logs;
pub mod os_assignments;
pub mod os_versions;
pub mod pagination;
pub mod pending_device_registrations;
pub mod pings;
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

pub(super) fn not_found(resource: &str, id: impl std::fmt::Display) -> Response {
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

pub(super) fn pagination_err(msg: &str) -> Response {
    err(StatusCode::UNPROCESSABLE_ENTITY, msg)
}

pub fn routes() -> Router {
    Router::new()
        .merge(application_assignments::routes())
        .merge(application_configs::routes())
        .merge(applications::routes())
        .merge(devices::routes())
        .merge(groups::routes())
        .merge(logs::routes())
        .merge(os_assignments::routes())
        .merge(os_versions::routes())
        .merge(pending_device_registrations::routes())
        .merge(pings::routes())
        .merge(reported_application_assignments::routes())
        .merge(reported_os_assignments::routes())
        .merge(tenants::routes())
        .merge(audit_log::routes())
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
        crate::api_v1::db::initialialize_db(
            "sqlite::memory:".into(),
            crate::config::AuditConfig::default(),
        )
        .await
        .unwrap();
        Router::new().nest("/v1", routes())
    }

    async fn test_app_postgres() -> (
        Router,
        testcontainers::ContainerAsync<testcontainers_modules::postgres::Postgres>,
    ) {
        use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
        let container = Postgres::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(5432).await.unwrap();
        let url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

        crate::api_v1::db::initialialize_db(url, crate::config::AuditConfig::default())
            .await
            .unwrap();

        (Router::new().nest("/v1", routes()), container)
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

    async fn put(app: Router, uri: &str, json: &str) -> (StatusCode, String) {
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
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
    async fn test_list_tenants_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/tenants").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_tenants_name_filter_returns_matching_items() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Acme Corp","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Acme Ltd","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Other","description":null}"#,
        )
        .await;
        let (_, body) = get(app, "/v1/tenants?name=Acme").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total_items"], 2);
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
    async fn test_list_groups_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/groups").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }
    #[tokio::test]
    #[serial]
    async fn test_list_groups_page_metadata_defaults() {
        let (_, body) = get(test_app().await, "/v1/groups").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["page"], 1);
        assert_eq!(json["page_size"], 20);
        assert_eq!(json["total_items"], 0);
        assert_eq!(json["total_pages"], 0);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_groups_paging_total_items_correct() {
        let app = test_app().await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G1"}"#).await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G2"}"#).await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G3"}"#).await;
        let (_, body) = get(app, "/v1/groups?page_size=2").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total_items"], 3);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_groups_page_zero_returns_422() {
        let (status, _) = get(test_app().await, "/v1/groups?page=0").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
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
    // Device::Model: { id: i32, uuid: String, serial_number: String, tenant_id: i32, group_id: Option<i32> }
    // tenant_id is required (non-optional) — must always be present in POST body.
    // A non-existent tenant_id will produce a 500 (FK violation), not 422.

    #[tokio::test]
    #[serial]
    async fn test_list_devices_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/devices").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_devices_page_size_one_returns_first_item_only() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"u1","serial_number":"h1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"u2","serial_number":"h2","tenant_id":1,"group_id":null}"#,
        )
        .await;
        let (_, body) = get(app, "/v1/devices?page_size=1").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_device_with_empty_uuid_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/devices",
            r#"{"id":0,"uuid":"","serial_number":"host-1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_device_with_empty_serial_number_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/devices",
            r#"{"id":0,"uuid":"some-uuid","serial_number":"","tenant_id":1,"group_id":null}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    // --- Device summaries ---

    #[tokio::test]
    #[serial]
    async fn test_list_device_summaries_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/devices/summary").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_device_summaries_total_items_correct() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"u1","serial_number":"h1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"u2","serial_number":"h2","tenant_id":1,"group_id":null}"#,
        )
        .await;
        let (_, body) = get(app, "/v1/devices/summary").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total_items"], 2);
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
    async fn test_list_applications_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/applications").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_applications_name_filter_returns_matching_items() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"nginx","description":"web server"}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"postgres","description":"database"}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"nginx-exporter","description":"metrics"}"#,
        )
        .await;
        let (_, body) = get(app, "/v1/applications?name=nginx").await;
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total_items"], 2);
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
    // ApplicationConfig::Model: { id: i32, device_id: Option<i32>, group_id: Option<i32>,
    //                             application_id: i32, image: String, config: String, version: i32 }
    // unique on (device_id, application_id); exactly one of device_id/group_id must be set.

    #[tokio::test]
    #[serial]
    async fn test_create_app_config_with_empty_image_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/app-configs",
            r#"{"device_id":1,"group_id":null,"application_id":1,"image":"","config":"x","version":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_app_config_without_device_or_group_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/app-configs",
            r#"{"device_id":null,"group_id":null,"application_id":1,"image":"img","config":"x","version":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_app_config_with_both_device_and_group_returns_422() {
        let (status, _) = post(
            test_app().await,
            "/v1/app-configs",
            r#"{"device_id":1,"group_id":1,"application_id":1,"image":"img","config":"x","version":1}"#,
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_app_configs_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/app-configs").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_create_app_config_returns_201_with_default_version() {
        let app = test_app().await;
        let (_, tenant_body) = post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Acme","description":null}"#,
        )
        .await;
        let tenant: serde_json::Value = serde_json::from_str(&tenant_body).unwrap();
        let (_, device_body) = post(
            app.clone(),
            "/v1/devices",
            &format!(
                r#"{{"id":0,"uuid":"dev-1","public_key":null,"serial_number":"host-1","tenant_id":{},"group_id":null}}"#,
                tenant["id"]
            ),
        )
        .await;
        let device: serde_json::Value = serde_json::from_str(&device_body).unwrap();
        let (_, app_body) = post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"my-app","description":"does things"}"#,
        )
        .await;
        let application: serde_json::Value = serde_json::from_str(&app_body).unwrap();

        let (status, body) = post(
            app,
            "/v1/app-configs",
            &format!(
                r#"{{"device_id":{},"group_id":null,"application_id":{},"image":"app:1"}}"#,
                device["id"], application["id"]
            ),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["config"], serde_json::Value::Null);
        // version omitted in the request body — server applies the default
        assert_eq!(json["version"], 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_update_app_config_changes_config_and_version() {
        let app = test_app().await;
        let (_, tenant_body) = post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Acme","description":null}"#,
        )
        .await;
        let tenant: serde_json::Value = serde_json::from_str(&tenant_body).unwrap();
        let (_, device_body) = post(
            app.clone(),
            "/v1/devices",
            &format!(
                r#"{{"id":0,"uuid":"dev-1","public_key":null,"serial_number":"host-1","tenant_id":{},"group_id":null}}"#,
                tenant["id"]
            ),
        )
        .await;
        let device: serde_json::Value = serde_json::from_str(&device_body).unwrap();
        let (_, app_body) = post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"my-app","description":"does things"}"#,
        )
        .await;
        let application: serde_json::Value = serde_json::from_str(&app_body).unwrap();

        let first_config = serde_json::json!({
            "environment": {
                "foo": "bar"
            }
        });
        let create_payload = serde_json::json!({
            "device_id": device["id"],
            "group_id": null,
            "application_id": application["id"],
            "image": "app:1",
            "config": first_config,
        })
        .to_string();

        let (_, created_body) = post(app.clone(), "/v1/app-configs", &create_payload).await;
        let created: serde_json::Value = serde_json::from_str(&created_body).unwrap();

        let update_config = serde_json::json!({
            "environment": {
                "bar": "baz"
            }
        });
        let update_payload = serde_json::json!({
            "device_id": device["id"],
            "group_id": null,
            "application_id": application["id"],
            "image": "app:1",
            "config": update_config,
        })
        .to_string();
        let (status, body) = put(
            app,
            &format!("/v1/app-configs/{}", created["id"]),
            &update_payload,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["config"], update_config);
        assert_eq!(json["version"], 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_app_configs_by_device_uuid_device_supersedes_group() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Acme","description":null}"#,
        )
        .await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G1"}"#).await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"dev-1","serial_number":"host-1","tenant_id":1,"group_id":1}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"my-app","description":"does things"}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/app-configs",
            r#"{"device_id":null,"group_id":1,"application_id":1,"image":"app:group","version":1}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/app-configs",
            r#"{"device_id":1,"group_id":null,"application_id":1,"image":"app:device","version":1}"#,
        )
        .await;

        let (status, body) = get(app, "/v1/app-configs?device_uuid=dev-1").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let data = json["data"].as_array().unwrap();
        assert_eq!(data.len(), 1, "device config should supersede group config");
        assert_eq!(data[0]["image"], "app:device");
    }

    // --- OS Versions ---
    // OsVersion::Model: { id: i32, commit_hash: String, orchestrator_version: String, description: Option<String> }
    // description IS optional here (unlike Application).

    #[tokio::test]
    #[serial]
    async fn test_list_os_versions_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/os-versions").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_os_versions_page_zero_returns_422() {
        let (status, _) = get(test_app().await, "/v1/os-versions?page=0").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

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

    #[tokio::test]
    #[serial]
    async fn test_list_app_assignments_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/app-assignments").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_os_assignments_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/os-assignments").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_app_assignments_by_device_uuid_device_direct_supersedes_group_duplicate() {
        // Scenario: a device belongs to a group. The same application_config_id is assigned
        // both to the device directly and to the group. The device-direct assignment must win
        // and only one entry should be returned (no duplicate).
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G1"}"#).await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"dev-dup-1","serial_number":"h1","tenant_id":1,"group_id":1}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"my-app","description":"desc"}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/app-configs",
            r#"{"device_id":1,"group_id":null,"application_id":1,"image":"app:device"}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/app-configs",
            r#"{"device_id":null,"group_id":1,"application_id":1,"image":"app:group"}"#,
        )
        .await;
        // app-config 1 assigned directly to device 1
        post(
            app.clone(),
            "/v1/app-assignments",
            r#"{"application_config_id":1,"device_id":1,"group_id":null}"#,
        )
        .await;
        // app-config 1 also assigned to group 1 (same application_config_id — the duplicate)
        post(
            app.clone(),
            "/v1/app-assignments",
            r#"{"application_config_id":1,"device_id":null,"group_id":1}"#,
        )
        .await;

        let (status, body) = get(app, "/v1/app-assignments?device_uuid=dev-dup-1").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let data = json["data"].as_array().unwrap();
        assert_eq!(
            data.len(),
            1,
            "duplicate app_config_id via group should be suppressed"
        );
        // The surviving assignment must be the device-direct one (device_id set, group_id null)
        assert_eq!(
            data[0]["device_id"], 1,
            "device-direct assignment should win over group assignment"
        );
        assert_eq!(
            data[0]["group_id"],
            serde_json::Value::Null,
            "group assignment should have been deduplicated away"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_list_os_assignments_by_device_uuid_device_direct_supersedes_group() {
        // Scenario: a device belongs to a group. Both have an OS assignment (possibly
        // different OS versions). The device-direct assignment must be the only result.
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G1"}"#).await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"dev-os-1","serial_number":"h1","tenant_id":1,"group_id":1}"#,
        )
        .await;
        // Two distinct OS versions so we can tell which assignment won
        post(
            app.clone(),
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"device-hash","orchestrator_version":"1.0","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"group-hash","orchestrator_version":"1.0","description":null}"#,
        )
        .await;
        // Group assignment: os-version 2
        post(
            app.clone(),
            "/v1/os-assignments",
            r#"{"os_version_id":2,"device_id":null,"group_id":1}"#,
        )
        .await;
        // Device-direct assignment: os-version 1
        post(
            app.clone(),
            "/v1/os-assignments",
            r#"{"os_version_id":1,"device_id":1,"group_id":null}"#,
        )
        .await;

        let (status, body) = get(app, "/v1/os-assignments?device_uuid=dev-os-1").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let data = json["data"].as_array().unwrap();
        assert_eq!(
            data.len(),
            1,
            "only the winning OS assignment should be returned"
        );
        assert_eq!(
            data[0]["os_version_id"], 1,
            "device-direct OS assignment (os_version_id=1) must win over group assignment (os_version_id=2)"
        );
        assert_eq!(
            data[0]["device_id"], 1,
            "winning assignment should have device_id set"
        );
        assert_eq!(
            data[0]["group_id"],
            serde_json::Value::Null,
            "winning assignment should not be the group assignment"
        );
    }

    // --- Reported assignments ---
    // ReportedApplicationAssignment::Model: { id: i32, application_config_id: i32, device_id: i32, updated_at: DateTimeUtc }
    // ReportedOsAssignment::Model:          { id: i32, os_version_id: i32, device_id: i32, updated_at: DateTimeUtc }
    // device_id is NOT optional on reported assignments (i32, not Option<i32>).
    // Both reported endpoints support POST from devices: body device_id or ?device_uuid=<str> query param.

    #[tokio::test]
    #[serial]
    async fn test_create_reported_app_assignment_with_device_uuid_returns_201() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"app-uuid-7","serial_number":"host-1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/applications",
            r#"{"id":0,"name":"my-app","description":"desc"}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/app-configs",
            r#"{"device_id":1,"group_id":null,"application_id":1,"image":"ghcr.io/example/app:1"}"#,
        )
        .await;

        let (status, body) = post(
            app,
            "/v1/reported-app-assignments?device_uuid=app-uuid-7",
            r#"{"application_config_id":1}"#,
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["application_config_id"], 1);
        assert_eq!(json["device_id"], 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_reported_os_assignments_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/reported-os-assignments").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_reported_app_assignments_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/reported-app-assignments").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_create_reported_os_assignment_with_device_id_returns_201() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"dev-uuid-1","serial_number":"host-1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"abc123","orchestrator_version":"1.0","description":null}"#,
        )
        .await;

        let (status, body) = post(
            app,
            "/v1/reported-os-assignments",
            r#"{"id":0,"os_version_id":1,"device_id":1,"updated_at":"2024-01-01T00:00:00Z"}"#,
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["os_version_id"], 1);
        assert_eq!(json["device_id"], 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_reported_os_assignment_with_device_uuid_returns_201() {
        let app = test_app().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"test-uuid-42","serial_number":"host-1","tenant_id":1,"group_id":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/os-versions",
            r#"{"id":0,"commit_hash":"abc123","orchestrator_version":"1.0","description":null}"#,
        )
        .await;

        let (status, body) = post(
            app,
            "/v1/reported-os-assignments?device_uuid=test-uuid-42",
            r#"{"id":0,"os_version_id":1,"device_id":0,"updated_at":"2024-01-01T00:00:00Z"}"#,
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["os_version_id"], 1);
        assert_eq!(json["device_id"], 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_create_reported_os_assignment_with_unknown_device_uuid_returns_404() {
        let (status, _) = post(
            test_app().await,
            "/v1/reported-os-assignments?device_uuid=does-not-exist",
            r#"{"id":0,"os_version_id":1,"device_id":0,"updated_at":"2024-01-01T00:00:00Z"}"#,
        )
        .await;

        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    // --- Audit logs ---
    // AuditLog::Model: { id: i32, table_name: String, record_id: String, operation: String,
    //                    old_data: Option<String>, new_data: Option<String>,
    //                    changed_by: i32, changed_at: DateTimeUtc }

    #[tokio::test]
    #[serial]
    async fn test_list_audit_logs_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/audit-logs").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    #[tokio::test]
    #[serial]
    async fn test_list_audit_logs_page_zero_returns_422() {
        let (status, _) = get(test_app().await, "/v1/audit-logs?page=0").await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    #[serial]
    async fn test_get_audit_logs_for_record_not_found_returns_404() {
        let (status, body) = get(test_app().await, "/v1/audit-logs/tenants/999").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(json.get("error").is_some());
    }

    #[tokio::test]
    #[serial]
    async fn test_get_audit_logs_for_device_returns_200_with_page_envelope() {
        let (status, body) = get(test_app().await, "/v1/audit-logs/by-device/1").await;
        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["data"], serde_json::json!([]));
    }

    // --- Audit logs (PostgreSQL-only) ---
    // Run with: cargo test -- --ignored (against a PostgreSQL instance)

    #[tokio::test]
    #[serial]
    async fn test_get_audit_logs_for_record_returns_entries_after_insert() {
        let (app, _container) = test_app_postgres().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"Acme","description":null}"#,
        )
        .await;

        let (status, body) = get(app, "/v1/audit-logs/tenants/1").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        let entries = json.as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["table_name"], "tenants");
        assert_eq!(entries[0]["record_id"], "1");
        assert_eq!(entries[0]["operation"], "INSERT");
    }

    #[tokio::test]
    #[serial]
    async fn test_list_audit_logs_filters_by_table_name() {
        let (app, _container) = test_app_postgres().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(app.clone(), "/v1/groups", r#"{"id":0,"name":"G"}"#).await;

        let (status, body) = get(app, "/v1/audit-logs?table_name=groups").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["total_items"], 1);
        assert_eq!(json["data"][0]["table_name"], "groups");
    }

    #[tokio::test]
    #[serial]
    async fn test_get_audit_logs_for_device_includes_device_history() {
        let (app, _container) = test_app_postgres().await;
        post(
            app.clone(),
            "/v1/tenants",
            r#"{"id":0,"name":"T","description":null}"#,
        )
        .await;
        post(
            app.clone(),
            "/v1/devices",
            r#"{"id":0,"uuid":"u1","serial_number":"h1","tenant_id":1,"group_id":null}"#,
        )
        .await;

        let (status, body) = get(app, "/v1/audit-logs/by-device/1").await;

        assert_eq!(status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert!(
            json["data"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["table_name"] == "devices")
        );
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
