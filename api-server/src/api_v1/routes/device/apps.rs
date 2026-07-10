use amos_common::entities::ApplicationConfig;
use axum::{Json, http::StatusCode, response::IntoResponse};

use crate::auth::extractors::AuthDevice;

/// GET /device/apps - Get the assigned applications
pub async fn get(AuthDevice(device): AuthDevice) -> Result<impl IntoResponse, StatusCode> {
    let apps = match apps_get_assigned(device.id, device.group_id).await {
        Ok(r) => r,
        Err(e) => {
            log::error!("{:?}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    Ok(Json(
        apps.into_iter()
            .map(|a| amos_common::device_api::apps::GetResponseItem {
                id: a.id,
                application_id: a.application_id,
                image: a.image,
                config: a.config,
            })
            .collect::<Vec<_>>(),
    ))
}

/// PUT /device/apps - Report the currently running applications
pub async fn put(
    AuthDevice(device): AuthDevice,
    Json(body): Json<amos_common::device_api::apps::PutBody>,
) -> StatusCode {
    let config_ids = body.into_iter().map(|item| item.application_config_id);
    match apps_put_report(device.id, config_ids).await {
        Ok(_) => StatusCode::CREATED,
        Err(e) => {
            log::error!("{:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

async fn apps_get_assigned(
    device_id: i32,
    group_id: Option<i32>,
) -> Result<Vec<ApplicationConfig::Model>, sea_orm::DbErr> {
    let (assignments, _) = crate::api_v1::db::list_application_assignments_for_device(
        device_id,
        group_id,
        None,
        0,
        u64::MAX,
    )
    .await?;

    let mut configs = Vec::with_capacity(assignments.len());
    for assignment in assignments {
        if let Some(config) =
            crate::api_v1::db::get_application_config(assignment.application_config_id).await?
        {
            configs.push(config);
        }
    }

    Ok(configs)
}

async fn apps_put_report(
    device_id: i32,
    application_config_ids: impl Iterator<Item = i32>,
) -> Result<(), sea_orm::DbErr> {
    for config_id in application_config_ids {
        crate::api_v1::db::add_reported_application_assignment(config_id, device_id).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use crate::api_v1::db;
    use crate::config::AuditConfig;

    // Regression test: assigned apps used to be resolved straight off
    // application_configs.device_id, bypassing application_assignments, so
    // deleting an assignment had no effect on what the device was told to run.
    #[tokio::test]
    #[serial]
    async fn apps_get_assigned_reflects_assignment_deletion() {
        db::initialialize_db("sqlite::memory:".into(), AuditConfig::default())
            .await
            .unwrap();

        let tenant = db::add_tenant("T".to_owned(), None).await.unwrap();
        let device = db::add_device("uuid".to_owned(), None, "host".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let app = db::add_application("App".to_owned(), "desc".to_owned())
            .await
            .unwrap();
        let config = db::add_application_config(
            Some(device.id),
            None,
            app.id,
            "quay.io/app".to_owned(),
            None,
        )
        .await
        .unwrap();
        let assignment = db::add_application_assignment_to_device(config.id, device.id)
            .await
            .unwrap();

        let assigned = super::apps_get_assigned(device.id, None).await.unwrap();
        assert_eq!(assigned.len(), 1);
        assert_eq!(assigned[0].id, config.id);

        db::delete_application_assignment(assignment.id)
            .await
            .unwrap();

        let assigned = super::apps_get_assigned(device.id, None).await.unwrap();
        assert!(
            assigned.is_empty(),
            "deleted assignment must no longer be reported as assigned"
        );
    }
}
