use jsonwebtoken::{DecodingKey, TokenData, Validation, decode};
use serde_json::Value;

use crate::api_v1::db;
use amos_common::device_jwt::{Claims, MAX_TOKEN_LIFETIME};

#[derive(Debug, Clone)]
pub struct ClientDevice {
    pub id: i32,
    pub group_id: Option<i32>,
}

/// Custom Error enum for distinguishing errors during JWT validation.
/// Specifically, we want to know the DeviceNotFound variant in the caller.
#[derive(Debug)]
pub enum DeviceTokenError {
    Jwt(()),
    DeviceNotFound,
    MissingPublicKey,
    DatabaseError(()),
}

impl From<jsonwebtoken::errors::Error> for DeviceTokenError {
    fn from(_e: jsonwebtoken::errors::Error) -> Self {
        DeviceTokenError::Jwt(())
    }
}

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub async fn validate_token(
    token: String,
    token_data: TokenData<Value>,
) -> Result<ClientDevice, DeviceTokenError> {
    let device_uuid = super::extract_claim(&token_data.claims, "sub")?;

    let device = db::get_device_by_uuid(device_uuid.clone())
        .await
        .map_err(|_| DeviceTokenError::DatabaseError(()))?;

    if device.is_none() {
        return Err(DeviceTokenError::DeviceNotFound);
    }

    let device_pubkey = device
        .clone()
        .unwrap()
        .public_key
        .ok_or(DeviceTokenError::MissingPublicKey)?;
    let device_pubkey_decoded = device_pubkey.replace("\\n", "\n");
    log::trace!(
        "Retrieved JWT pubkey for device {}: {}",
        device_uuid,
        device_pubkey_decoded
    );

    let verified_token = decode::<Claims>(
        token,
        &DecodingKey::from_rsa_pem(device_pubkey_decoded.as_bytes())?,
        &Validation::new(jsonwebtoken::Algorithm::RS256),
    )?;

    // Ensure tokens is not issued for longer than their maximum lifetime
    // Allow some drift due to (missing) clock synchronization
    let difference_secs = verified_token.claims.exp - chrono::Utc::now().timestamp();
    if (difference_secs - 10) > MAX_TOKEN_LIFETIME {
        log::warn!(
            "Rejected device JWT due to too long lifetime: {} secs",
            difference_secs
        );
        return Err(DeviceTokenError::Jwt(()));
    }

    let dev = device.unwrap();
    Ok(ClientDevice {
        id: dev.id,
        group_id: dev.group_id,
    })
}
