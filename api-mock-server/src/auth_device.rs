use jsonwebtoken::{DecodingKey, TokenData, Validation, decode};
use log::trace;
use serde_json::Value;

use amos_common::device_jwt::Claims;
use crate::{api_v1::db, auth_user::get_str};

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub async fn validate_device_token(
    token: String,
    token_data: TokenData<Value>,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let device_uuid = get_str(&token_data.claims, "sub")?;

    let device = db::get_device_by_uuid(device_uuid.clone())
        .await
        .map_err(|_| jsonwebtoken::errors::Error::from(
            jsonwebtoken::errors::ErrorKind::InvalidToken
        ))?
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    let device_pubkey = device.public_key.ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let device_pubkey_decoded = device_pubkey.replace("\\n", "\n");
    trace!("Retrieved JWT pubkey for device {}: {}", device_uuid, device_pubkey_decoded);

    let verified_token = decode::<Claims>(
        token,
        &DecodingKey::from_rsa_pem(device_pubkey_decoded.as_bytes())?,
        &Validation::new(jsonwebtoken::Algorithm::RS256),
    )?;

    Ok(verified_token.claims)
}
