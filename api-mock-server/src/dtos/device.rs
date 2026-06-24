use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use super::{Group, Tenant};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "devices")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub id: i32,

    pub uuid: String,

    pub public_key: Option<String>,

    pub serial_number: String,

    pub tenant_id: i32,
    #[sea_orm(belongs_to, from = "tenant_id", to = "id")]
    pub tenant: HasOne<Tenant::Entity>,

    pub group_id: Option<i32>,
    #[sea_orm(belongs_to, from = "group_id", to = "id")]
    pub group: HasOne<Group::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub fn into_api(self) -> amos_common::entities::Device::Model {
        amos_common::entities::Device::Model {
            id: self.id,
            uuid: self.uuid,
            public_key: self.public_key,
            serial_number: self.serial_number,
            tenant_id: self.tenant_id,
            group_id: self.group_id,
        }
    }
}
