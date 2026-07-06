use sea_orm::ModelTrait;

impl super::DataStore {
    pub async fn register_device(
        &self,
        uuid: String,
        serial_number: String,
        endorsement_pubkey: String,
        signing_pubkey: String,
    ) -> Result<(), sea_orm::DbErr> {
        // Check if a matching pending registration is in the database
        let found = crate::api_v1::db::search_pending_device_registration(
            serial_number.clone(),
            endorsement_pubkey,
        )
        .await?;

        let active = match found {
            Some(x) => x,
            None => {
                return Err(sea_orm::DbErr::RecordNotFound(
                    "Did not find device registration".to_owned(),
                ));
            }
        };

        let new_device = crate::api_v1::db::add_device(
            uuid,
            Some(signing_pubkey),
            serial_number,
            1, // TODO: Having to guess a tenat here is BAD, tho not sure what else to do as it is mandatory
            None,
        )
        .await?;

        log::info!("New device registered successfully: {:?}", new_device);

        let _ = active.delete(&crate::api_v1::db::db!()).await;
        Ok(())
    }
}
