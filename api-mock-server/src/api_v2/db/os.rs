use amos_common::entities::OsVersion;

impl super::DataStore {
    pub async fn os_get_assigned(
        &self,
        device_id: i32,
        group_id: Option<i32>,
    ) -> Result<OsVersion::Model, sea_orm::DbErr> {
        let (assignments, _) = crate::api_v1::db::list_os_assignments_for_device(
            device_id,
            group_id,
            None,
            0,
            u64::MAX,
        )
        .await?;

        if assignments.is_empty() {
            return Err(sea_orm::DbErr::RecordNotFound("OsVersion".to_owned()));
        }

        match crate::api_v1::db::get_os_version(assignments[0].os_version_id).await? {
            Some(ver) => Ok(ver),
            None => Err(sea_orm::DbErr::RecordNotFound("OsVersion".to_owned())),
        }
    }

    pub async fn os_put_report(
        &self,
        device_id: i32,
        os_version_id: i32,
    ) -> Result<(), sea_orm::DbErr> {
        let _ = crate::api_v1::db::add_reported_os_assignment(os_version_id, device_id).await?;
        Ok(())
    }
}
