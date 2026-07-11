use crate::dtos;
use amos_common::entities::PendingDeviceRegistration;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
};

use super::db;

// --Pending device registrations--

pub async fn list_pending_device_registrations(
    page: u64,
    page_size: u64,
) -> Result<(Vec<PendingDeviceRegistration::Model>, u64), DbErr> {
    let db = db!();
    let query = dtos::PendingDeviceRegistration::Entity::find()
        .order_by_asc(dtos::PendingDeviceRegistration::Column::Id);
    let paginator = query.paginate(&db, page_size);
    let total_items = paginator.num_items().await?;
    let data = paginator.fetch_page(page).await?;
    Ok((
        data.into_iter().map(|m| m.into_api()).collect(),
        total_items,
    ))
}

pub async fn add_pending_device_registration(
    serial_number: String,
    endorsement_public_key: String,
) -> Result<PendingDeviceRegistration::Model, DbErr> {
    let pending_device_registration = dtos::PendingDeviceRegistration::ActiveModel {
        id: NotSet,
        serial_number: Set(serial_number),
        endorsement_public_key: Set(endorsement_public_key),
    };

    let db = db!();

    let new_pending_device_registration = pending_device_registration.insert(&db).await?;
    log::trace!(
        "Inserted new pending device registration: {:?}",
        new_pending_device_registration
    );
    Ok(new_pending_device_registration.into_api())
}

pub async fn delete_pending_device_registration(id: i32) -> Result<u64, DbErr> {
    let db = db!();
    let del = dtos::PendingDeviceRegistration::Entity::delete_by_id(id)
        .exec(&db)
        .await?;
    Ok(del.rows_affected)
}

pub async fn search_pending_device_registration(
    serial_number: String,
    endorsement_public_key: String,
) -> Result<Option<dtos::PendingDeviceRegistration::Model>, DbErr> {
    let db = db!();
    let result = dtos::PendingDeviceRegistration::Entity::find()
        .filter(dtos::PendingDeviceRegistration::Column::SerialNumber.eq(serial_number))
        .filter(
            dtos::PendingDeviceRegistration::Column::EndorsementPublicKey
                .eq(endorsement_public_key),
        )
        .one(&db)
        .await?;

    Ok(result)
}
