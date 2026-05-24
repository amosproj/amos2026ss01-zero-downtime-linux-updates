use log::debug;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectOptions, Database, DatabaseConnection, DbErr,
    EntityTrait, QueryFilter,
};
use sea_orm_migration::MigratorTrait;
use serde_json::json;
use tokio::sync::RwLock;

use crate::db_migration::Migrator;
use amos_common::entities::{
    Application, ApplicationAssignment, ApplicationConfig, Device, Group, OsAssignment, OsVersion,
    ReportedApplicationAssignment, ReportedOsAssignment, Tenant,
};

static DB: RwLock<Option<DatabaseConnection>> = RwLock::const_new(None);

macro_rules! db {
    () => {
        DB.read().await.clone().unwrap()
    };
}

pub async fn initialialize_db(database_url: String) -> Result<(), DbErr> {
    let mut opt = ConnectOptions::new(database_url.to_owned());
    // SQL queries should be the last resort when debugging...
    opt.sqlx_logging_level(log::LevelFilter::Trace);

    let conn = Database::connect(opt).await?;

    conn.ping().await?;

    Migrator::up(&conn, None).await?;

    DB.write().await.replace(conn);

    Ok(())
}

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

    let mut query = Device::Entity::find();
    if let Some(id) = device_id {
        query = query.filter(Device::Column::Id.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(Device::Column::TenantId.eq(id));
    }

    let devices = query.all(&db).await?;
    if devices.is_empty() {
        return Ok(vec![]);
    }

    let device_ids: Vec<i32> = devices.iter().map(|d| d.id).collect();

    let os_rows = ReportedOsAssignment::Entity::find()
        .filter(ReportedOsAssignment::Column::DeviceId.is_in(device_ids.clone()))
        .find_also_related(OsVersion::Entity)
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

    let app_rows = ReportedApplicationAssignment::Entity::find()
        .filter(ReportedApplicationAssignment::Column::DeviceId.is_in(device_ids.clone()))
        .find_also_related(ApplicationConfig::Entity)
        .all(&db)
        .await?;

    let app_ids: Vec<i32> = app_rows
        .iter()
        .filter_map(|(_, config)| config.as_ref().map(|c| c.application_id))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let app_by_id: std::collections::HashMap<i32, Application::Model> = Application::Entity::find()
        .filter(Application::Column::Id.is_in(app_ids))
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

    let tenant_by_id: std::collections::HashMap<i32, Tenant::Model> = Tenant::Entity::find()
        .filter(Tenant::Column::Id.is_in(tenant_ids))
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
                "device": device,
                "tenant": tenant,
                "os_versions": os,
                "applications": apps,
            })
        })
        .collect();

    Ok(summaries)
}

// --Tenants--

pub async fn list_tenants() -> Result<Vec<Tenant::Model>, DbErr> {
    let db = db!();
    Tenant::Entity::find().all(&db).await
}

pub async fn get_tenant(id: i32) -> Result<Option<Tenant::Model>, DbErr> {
    let db = db!();
    Tenant::Entity::find_by_id(id).one(&db).await
}

pub async fn add_tenant(name: String, description: Option<String>) -> Result<Tenant::Model, DbErr> {
    let tenant = Tenant::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_tenant = tenant.insert(&db).await?;
    debug!("Inserted new tenant: {:?}", new_tenant);

    Ok(new_tenant)
}

pub async fn update_tenant(
    id: i32,
    name: String,
    description: Option<String>,
) -> Result<Tenant::Model, DbErr> {
    let db = db!();
    let tenant = Tenant::ActiveModel {
        id: Set(id),
        name: Set(name),
        description: Set(description),
    };
    let updated_tenant = tenant.update(&db).await?;
    debug!("Updated tenant: {:?}", updated_tenant);
    Ok(updated_tenant)
}

pub async fn delete_tenant(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Tenant::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}

// --Groups--

pub async fn list_groups() -> Result<Vec<Group::Model>, DbErr> {
    let db = db!();
    Group::Entity::find().all(&db).await
}

pub async fn get_group(id: i32) -> Result<Option<Group::Model>, DbErr> {
    let db = db!();
    Group::Entity::find_by_id(id).one(&db).await
}

pub async fn add_group(name: String) -> Result<Group::Model, DbErr> {
    let group = Group::ActiveModel {
        id: NotSet,
        name: Set(name.to_owned()),
        // ..Default::default()
    };

    let db = db!();

    let new_group = group.insert(&db).await?;
    debug!("Inserted group: {:?}", new_group);

    Ok(new_group)
}

pub async fn update_group(id: i32, name: String) -> Result<Group::Model, DbErr> {
    let db = db!();
    let group = Group::ActiveModel {
        id: Set(id),
        name: Set(name),
    };
    let updated_group = group.update(&db).await?;
    debug!("Updated group: {:?}", updated_group);
    Ok(updated_group)
}

pub async fn delete_group(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Group::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}

// --Devices--

pub async fn list_devices(
    group_id: Option<i32>,
    tenant_id: Option<i32>,
) -> Result<Vec<Device::Model>, DbErr> {
    let db = db!();
    let mut query = Device::Entity::find();
    if let Some(id) = group_id {
        query = query.filter(Device::Column::GroupId.eq(id));
    }
    if let Some(id) = tenant_id {
        query = query.filter(Device::Column::TenantId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_device(id: i32) -> Result<Option<Device::Model>, DbErr> {
    let db = db!();
    Device::Entity::find_by_id(id).one(&db).await
}

pub async fn add_device(
    uuid: String,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let device = Device::ActiveModel {
        id: NotSet,
        uuid: Set(uuid),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_device = device.insert(&db).await?;
    debug!("Inserted device: {:?}", new_device);

    Ok(new_device)
}

pub async fn update_device(
    id: i32,
    uuid: String,
    hostname: String,
    tenant_id: i32,
    group_id: Option<i32>,
) -> Result<Device::Model, DbErr> {
    let db = db!();
    let device = Device::ActiveModel {
        id: Set(id),
        uuid: Set(uuid),
        hostname: Set(hostname),
        tenant_id: Set(tenant_id),
        group_id: Set(group_id),
    };
    let updated_device = device.update(&db).await?;
    debug!("Updated device: {:?}", updated_device);
    Ok(updated_device)
}

pub async fn delete_device(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Device::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}

// --Applications--

pub async fn list_applications() -> Result<Vec<Application::Model>, DbErr> {
    let db = db!();
    Application::Entity::find().all(&db).await
}

pub async fn get_application(id: i32) -> Result<Option<Application::Model>, DbErr> {
    let db = db!();
    Application::Entity::find_by_id(id).one(&db).await
}

pub async fn add_application(
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let app = Application::ActiveModel {
        id: NotSet,
        name: Set(name),
        description: Set(description),
    };

    let db = db!();

    let new_app = app.insert(&db).await?;
    debug!("Inserted new application: {:?}", new_app);

    Ok(new_app)
}

pub async fn update_application(
    id: i32,
    name: String,
    description: String,
) -> Result<Application::Model, DbErr> {
    let db = db!();
    let app = Application::ActiveModel {
        id: Set(id),
        name: Set(name),
        description: Set(description),
    };
    let updated_app = app.update(&db).await?;
    debug!("Updated application: {:?}", updated_app);
    Ok(updated_app)
}

pub async fn delete_application(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = Application::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}

// --Application Configs--

pub async fn list_application_configs(
    application_id: Option<i32>,
) -> Result<Vec<ApplicationConfig::Model>, DbErr> {
    let db = db!();
    let mut query = ApplicationConfig::Entity::find();
    if let Some(id) = application_id {
        query = query.filter(ApplicationConfig::Column::ApplicationId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_application_config(id: i32) -> Result<Option<ApplicationConfig::Model>, DbErr> {
    let db = db!();
    ApplicationConfig::Entity::find_by_id(id).one(&db).await
}

pub async fn add_application_config(
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let app_config = ApplicationConfig::ActiveModel {
        id: NotSet,
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
    };

    let db = db!();

    let new_app_config = app_config.insert(&db).await?;
    debug!("Inserted new application config: {:?}", new_app_config);

    Ok(new_app_config)
}

pub async fn update_application_config(
    id: i32,
    app_id: i32,
    image: String,
    config: Option<String>,
    comment: Option<String>,
) -> Result<ApplicationConfig::Model, DbErr> {
    let db = db!();
    let app_config = ApplicationConfig::ActiveModel {
        id: Set(id),
        application_id: Set(app_id),
        image: Set(image),
        config: Set(config),
        comment: Set(comment),
    };
    let updated_group = app_config.update(&db).await?;
    debug!("Updated application config: {:?}", updated_group);
    Ok(updated_group)
}

pub async fn delete_application_config(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ApplicationConfig::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}

// --Application Assignments--

pub async fn list_application_assignments(
    application_config_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<Vec<ApplicationAssignment::Model>, DbErr> {
    let db = db!();
    let mut query = ApplicationAssignment::Entity::find();
    if let Some(id) = application_config_id {
        query = query.filter(ApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(ApplicationAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(ApplicationAssignment::Column::GroupId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_application_assignment(
    id: i32,
) -> Result<Option<ApplicationAssignment::Model>, DbErr> {
    let db = db!();
    ApplicationAssignment::Entity::find_by_id(id).one(&db).await
}

pub async fn add_application_assignment(
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let app_assignment = ApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    debug!(
        "Inserted new application config assignment: {:?}",
        new_app_assignment
    );

    Ok(new_app_assignment)
}

pub async fn update_application_assignment(
    id: i32,
    app_config_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<ApplicationAssignment::Model, DbErr> {
    let db = db!();
    let app_assignment = ApplicationAssignment::ActiveModel {
        id: Set(id),
        application_config_id: Set(app_config_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };
    let updated_group = app_assignment.update(&db).await?;
    debug!("Updated application assignment: {:?}", updated_group);
    Ok(updated_group)
}

pub async fn delete_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ApplicationAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}

// --Reported Application Assignments--

pub async fn list_reported_application_assignments(
    device_id: Option<i32>,
    application_config_id: Option<i32>,
) -> Result<Vec<ReportedApplicationAssignment::Model>, DbErr> {
    let db = db!();
    let mut query = ReportedApplicationAssignment::Entity::find();
    if let Some(id) = device_id {
        query = query.filter(ReportedApplicationAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = application_config_id {
        query = query.filter(ReportedApplicationAssignment::Column::ApplicationConfigId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_reported_application_assignment(
    id: i32,
) -> Result<Option<ReportedApplicationAssignment::Model>, DbErr> {
    let db = db!();
    ReportedApplicationAssignment::Entity::find_by_id(id)
        .one(&db)
        .await
}

#[allow(dead_code)]
pub async fn add_reported_application_assignment(
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let app_assignment = ReportedApplicationAssignment::ActiveModel {
        id: NotSet,
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };

    let db = db!();

    let new_app_assignment = app_assignment.insert(&db).await?;
    debug!(
        "Inserted new reported application assignment: {:?}",
        new_app_assignment
    );
    Ok(new_app_assignment)
}

#[allow(dead_code)]
pub async fn update_reported_application_assignment(
    id: i32,
    application_config_id: i32,
    device_id: i32,
) -> Result<ReportedApplicationAssignment::Model, DbErr> {
    let db = db!();
    let app_assignment = ReportedApplicationAssignment::ActiveModel {
        id: Set(id),
        application_config_id: Set(application_config_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };
    let updated_group = app_assignment.update(&db).await?;
    debug!(
        "Updated reported application assignment: {:?}",
        updated_group
    );
    Ok(updated_group)
}

pub async fn delete_reported_application_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ReportedApplicationAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}

// --OS Versions--

pub async fn list_os_versions() -> Result<Vec<OsVersion::Model>, DbErr> {
    let db = db!();
    OsVersion::Entity::find().all(&db).await
}

pub async fn get_os_version(id: i32) -> Result<Option<OsVersion::Model>, DbErr> {
    let db = db!();
    OsVersion::Entity::find_by_id(id).one(&db).await
}

pub async fn add_os_version(
    commit_hash: String,
    orchestrator_version: String,
    description: Option<String>,
) -> Result<OsVersion::Model, DbErr> {
    let os_version = OsVersion::ActiveModel {
        id: NotSet,
        commit_hash: Set(commit_hash),
        orchestrator_version: Set(orchestrator_version),
        description: Set(description),
    };

    let db = db!();

    let new_os_version = os_version.insert(&db).await?;
    debug!("Inserted new OS version: {:?}", new_os_version);

    Ok(new_os_version)
}

pub async fn update_os_version(
    id: i32,
    commit_hash: String,
    orchestrator_version: String,
    description: Option<String>,
) -> Result<OsVersion::Model, DbErr> {
    let db = db!();
    let os_version = OsVersion::ActiveModel {
        id: Set(id),
        commit_hash: Set(commit_hash),
        orchestrator_version: Set(orchestrator_version),
        description: Set(description),
    };
    let updated_os_version = os_version.update(&db).await?;
    debug!("Updated OS version: {:?}", updated_os_version);
    Ok(updated_os_version)
}

pub async fn delete_os_version(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = OsVersion::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}

// --OS Assignments--

pub async fn list_os_assignments(
    os_version_id: Option<i32>,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<Vec<OsAssignment::Model>, DbErr> {
    let db = db!();
    let mut query = OsAssignment::Entity::find();
    if let Some(id) = os_version_id {
        query = query.filter(OsAssignment::Column::OsVersionId.eq(id));
    }
    if let Some(id) = device_id {
        query = query.filter(OsAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = group_id {
        query = query.filter(OsAssignment::Column::GroupId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_os_assignment(id: i32) -> Result<Option<OsAssignment::Model>, DbErr> {
    let db = db!();
    OsAssignment::Entity::find_by_id(id).one(&db).await
}

pub async fn add_os_assignment(
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<OsAssignment::Model, DbErr> {
    let os_assignment = OsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };

    let db = db!();

    let new_os_assignment = os_assignment.insert(&db).await?;
    debug!(
        "Inserted new OS version assignment: {:?}",
        new_os_assignment
    );

    Ok(new_os_assignment)
}

pub async fn update_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: Option<i32>,
    group_id: Option<i32>,
) -> Result<OsAssignment::Model, DbErr> {
    let db = db!();
    let os_assignment = OsAssignment::ActiveModel {
        id: Set(id),
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        group_id: Set(group_id),
    };
    let updated_os_assignment = os_assignment.update(&db).await?;
    debug!("Updated OS version assignment: {:?}", updated_os_assignment);
    Ok(updated_os_assignment)
}

pub async fn delete_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = OsAssignment::Entity::delete_by_id(id).exec(&db).await?;
    Ok(del.rows_affected)
}

// --Reported OS Assignments--

pub async fn list_reported_os_assignments(
    device_id: Option<i32>,
    os_version_id: Option<i32>,
) -> Result<Vec<ReportedOsAssignment::Model>, DbErr> {
    let db = db!();
    let mut query = ReportedOsAssignment::Entity::find();
    if let Some(id) = device_id {
        query = query.filter(ReportedOsAssignment::Column::DeviceId.eq(id));
    }
    if let Some(id) = os_version_id {
        query = query.filter(ReportedOsAssignment::Column::OsVersionId.eq(id));
    }
    query.all(&db).await
}

pub async fn get_reported_os_assignment(
    id: i32,
) -> Result<Option<ReportedOsAssignment::Model>, DbErr> {
    let db = db!();
    ReportedOsAssignment::Entity::find_by_id(id).one(&db).await
}

#[allow(dead_code)]
pub async fn add_reported_os_assignment(
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let os_assignment = ReportedOsAssignment::ActiveModel {
        id: NotSet,
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };

    let db = db!();

    let new_os_assignment = os_assignment.insert(&db).await?;
    debug!(
        "Inserted new reported OS version assignment: {:?}",
        new_os_assignment
    );
    Ok(new_os_assignment)
}

#[allow(dead_code)]
pub async fn update_reported_os_assignment(
    id: i32,
    os_version_id: i32,
    device_id: i32,
) -> Result<ReportedOsAssignment::Model, DbErr> {
    let db = db!();
    let os_assignment = ReportedOsAssignment::ActiveModel {
        id: Set(id),
        os_version_id: Set(os_version_id),
        device_id: Set(device_id),
        updated_at: NotSet, // update_at is automatically set in before_save
    };
    let updated_os_assignment = os_assignment.update(&db).await?;
    debug!(
        "Updated reported OS version assignment: {:?}",
        updated_os_assignment
    );
    Ok(updated_os_assignment)
}

pub async fn delete_reported_os_assignment(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = ReportedOsAssignment::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}

#[allow(dead_code)]
pub async fn add_application_assignment_to_device(
    app_config_id: i32,
    device_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, Some(device_id), None).await
}

#[allow(dead_code)]
pub async fn add_application_assignment_to_group(
    app_config_id: i32,
    group_id: i32,
) -> Result<ApplicationAssignment::Model, DbErr> {
    add_application_assignment(app_config_id, None, Some(group_id)).await
}

#[cfg(test)]
mod tests {
    use sea_orm::sea_query::prelude::serde_json;
    use serial_test::serial;

    #[cfg(test)]
    async fn test_initialize_empty_inmem_db() {
        super::initialialize_db("sqlite::memory:".into())
            .await
            .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_initialize_db_succeeds() {
        test_initialize_empty_inmem_db().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_device_with_existing_group_and_tenant_works() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant(
            "Kathis Käjsewelt".to_owned(),
            Some("Sitz: Nürnberg".to_owned()),
        )
        .await
        .unwrap();
        let group = super::add_group("Werk Erlangen #5".into()).await.unwrap();

        super::add_device(
            "c0ffee-xdxdxd-129874".to_owned(),
            "host-01.er5.weber.group".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_device_with_not_existing_group_fails() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("X".to_owned(), None).await.unwrap();

        let result = super::add_device(
            "c0ffee-xdxdxd-129874".to_owned(),
            "host-01.er5.weber.group".to_owned(),
            tenant.id,
            Some(0),
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_app_assignement_for_device_works() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("X".to_owned(), None).await.unwrap();

        let device = super::add_device("".to_owned(), "".to_owned(), tenant.id, None)
            .await
            .unwrap();
        println!("Created device: {:?}", device);

        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();
        println!("Created app: {:?}", app);

        let app_config =
            super::add_application_config(app.id, "quay.io/bla".to_owned(), None, None)
                .await
                .unwrap();
        println!("Created app config: {:?}", app_config);

        let result = super::add_application_assignment_to_device(app_config.id, device.id).await;
        println!("Created application assignment: {:?}", result);

        assert!(result.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_insert_app_assignement_without_group_or_device_fails() {
        test_initialize_empty_inmem_db().await;

        let app = super::add_application("App 1".to_owned(), "Sample app".to_owned())
            .await
            .unwrap();
        println!("Created app: {:?}", app);

        let app_config =
            super::add_application_config(app.id, "quay.io/bla".to_owned(), None, None)
                .await
                .unwrap();
        println!("Created app config: {:?}", app_config);

        let result = super::add_application_assignment(app_config.id, None, None).await;
        println!("Created application assignment: {:?}", result);

        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial]
    async fn generated_application_model_json_matches_expected() {
        test_initialize_empty_inmem_db().await;

        let app = super::add_application("app-a".to_owned(), "cool app".to_owned())
            .await
            .unwrap();
        let app_json = serde_json::to_string(&app).unwrap();

        let expected = r#"{"id":1,"name":"app-a","description":"cool app"}"#;
        assert_eq!(app_json, expected);
    }

    #[tokio::test]
    #[serial]
    async fn given_application_model_json_unmarshalls() {
        test_initialize_empty_inmem_db().await;

        let app_json = r#"{"id":5,"name":"app-b","description":"mediocre app"}"#;
        let app: Result<super::Application::Model, _> = serde_json::from_str(app_json);

        assert!(app.is_ok());
    }

    #[tokio::test]
    #[serial]
    async fn test_list_devices_filtered_by_tenant() {
        test_initialize_empty_inmem_db().await;

        let t1 = super::add_tenant("T1".to_owned(), None).await.unwrap();
        let t2 = super::add_tenant("T2".to_owned(), None).await.unwrap();
        super::add_device("uuid-1".to_owned(), "host-1".to_owned(), t1.id, None)
            .await
            .unwrap();
        super::add_device("uuid-2".to_owned(), "host-2".to_owned(), t1.id, None)
            .await
            .unwrap();
        super::add_device("uuid-3".to_owned(), "host-3".to_owned(), t2.id, None)
            .await
            .unwrap();
        let devices = super::list_devices(None, Some(t1.id)).await.unwrap();

        assert_eq!(devices.len(), 2);
    }

    #[tokio::test]
    #[serial]
    async fn test_list_devices_filtered_by_group() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let group = super::add_group("G".to_owned()).await.unwrap();
        super::add_device(
            "uuid-1".to_owned(),
            "host-1".to_owned(),
            tenant.id,
            Some(group.id),
        )
        .await
        .unwrap();
        super::add_device("uuid-2".to_owned(), "host-2".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let devices = super::list_devices(Some(group.id), None).await.unwrap();

        assert_eq!(devices.len(), 1);
    }

    #[tokio::test]
    #[serial]
    async fn test_application_assignment_reassign_to_different_device() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let d1 = super::add_device("uuid-d1".to_owned(), "host-d1".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let d2 = super::add_device("uuid-d2".to_owned(), "host-d2".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let app = super::add_application("app".to_owned(), "desc".to_owned())
            .await
            .unwrap();
        let config =
            super::add_application_config(app.id, "quay.io/app:1.0".to_owned(), None, None)
                .await
                .unwrap();
        let assignment = super::add_application_assignment(config.id, Some(d1.id), None)
            .await
            .unwrap();
        let updated =
            super::update_application_assignment(assignment.id, config.id, Some(d2.id), None)
                .await
                .unwrap();

        assert_eq!(updated.device_id, Some(d2.id));
    }

    #[tokio::test]
    #[serial]
    async fn test_device_update_changes_hostname() {
        test_initialize_empty_inmem_db().await;

        let tenant = super::add_tenant("T".to_owned(), None).await.unwrap();
        let device = super::add_device("uuid".to_owned(), "old-host".to_owned(), tenant.id, None)
            .await
            .unwrap();
        let updated = super::update_device(
            device.id,
            device.uuid,
            "new-hostname".to_owned(),
            tenant.id,
            None,
        )
        .await
        .unwrap();

        assert_eq!(updated.hostname, "new-hostname");
    }

    #[tokio::test]
    #[serial]
    async fn test_application_update_changes_description() {
        test_initialize_empty_inmem_db().await;

        let app = super::add_application("app".to_owned(), "old".to_owned())
            .await
            .unwrap();
        let updated = super::update_application(app.id, "app".to_owned(), "new".to_owned())
            .await
            .unwrap();

        assert_eq!(updated.description, "new");
    }

    #[tokio::test]
    #[serial]
    async fn test_device_summary_shape_and_contents() {
        test_initialize_empty_inmem_db().await;

        // -- Arrange --
        let tenant = super::add_tenant("Acme".to_owned(), Some("Sitz: Nürnberg".to_owned()))
            .await
            .unwrap();
        let device =
            super::add_device("uuid-abc".to_owned(), "host-01".to_owned(), tenant.id, None)
                .await
                .unwrap();

        // OS side
        let os_version = super::add_os_version(
            "deadbeef".to_owned(),
            "1.2.3".to_owned(),
            Some("stable release".to_owned()),
        )
        .await
        .unwrap();
        super::add_reported_os_assignment(os_version.id, device.id)
            .await
            .unwrap();

        // App side
        let app = super::add_application("my-app".to_owned(), "does things".to_owned())
            .await
            .unwrap();
        let config = super::add_application_config(
            app.id,
            "quay.io/my-app:1.0".to_owned(),
            Some(r#"{"port":8080}"#.to_owned()),
            Some("primary instance".to_owned()),
        )
        .await
        .unwrap();
        super::add_reported_application_assignment(config.id, device.id)
            .await
            .unwrap();

        // -- Act --
        let summary = super::get_device_summary(device.id)
            .await
            .unwrap()
            .expect("summary should exist for a known device id");

        // -- Assert: top-level keys --
        assert!(summary.get("device").is_some());
        assert!(summary.get("tenant").is_some());
        assert!(summary.get("os_versions").is_some());
        assert!(summary.get("applications").is_some());

        // -- Assert: device fields --
        assert_eq!(summary["device"]["uuid"], "uuid-abc");
        assert_eq!(summary["device"]["hostname"], "host-01");
        assert_eq!(summary["device"]["tenant_id"], tenant.id);
        assert_eq!(summary["device"]["group_id"], serde_json::Value::Null);

        // -- Assert: tenant fields --
        assert_eq!(summary["tenant"]["name"], "Acme");
        assert_eq!(summary["tenant"]["description"], "Sitz: Nürnberg");

        // -- Assert: os_versions entry --
        let os_versions = summary["os_versions"].as_array().unwrap();
        assert_eq!(os_versions.len(), 1);
        let os_entry = &os_versions[0];
        assert_eq!(os_entry["commit_hash"], "deadbeef");
        assert_eq!(os_entry["orchestrator_version"], "1.2.3");
        assert_eq!(os_entry["description"], "stable release");
        assert!(os_entry.get("reported_assignment_id").is_some());
        assert!(os_entry.get("updated_at").is_some());

        // -- Assert: applications entry --
        let applications = summary["applications"].as_array().unwrap();
        assert_eq!(applications.len(), 1);
        let app_entry = &applications[0];
        assert_eq!(app_entry["application_name"], "my-app");
        assert_eq!(app_entry["application_description"], "does things");
        assert_eq!(app_entry["image"], "quay.io/my-app:1.0");
        assert_eq!(app_entry["config"], r#"{"port":8080}"#);
        assert_eq!(app_entry["comment"], "primary instance");
        assert!(app_entry.get("reported_assignment_id").is_some());
        assert!(app_entry.get("updated_at").is_some());
    }
}
