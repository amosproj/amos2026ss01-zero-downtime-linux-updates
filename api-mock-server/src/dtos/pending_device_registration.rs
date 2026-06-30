use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "pending_device_registrations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub serial_number: String,

    pub endorsement_public_key: String,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::PendingDeviceRegistration::Model {
        amos_common::entities::PendingDeviceRegistration::Model {
            id: self.id,
            serial_number: self.serial_number,
            endorsement_public_key: self.endorsement_public_key,
        }
    }
}
