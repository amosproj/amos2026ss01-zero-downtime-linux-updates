use jsonwebtoken::{DecodingKey, TokenData, Validation, decode};
use log::{trace, warn};
use serde_json::Value;

use crate::{api_v1::db, auth_user::get_str};
use amos_common::device_jwt::{Claims, MAX_TOKEN_LIFETIME};

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub async fn validate_device_token(
    token: String,
    token_data: TokenData<Value>,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let device_uuid = get_str(&token_data.claims, "sub")?;

    let device = db::get_device_by_uuid(device_uuid.clone())
        .await
        .map_err(|_| {
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken)
        })?
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;

    let device_pubkey = device
        .public_key
        .ok_or(jsonwebtoken::errors::ErrorKind::InvalidToken)?;
    let device_pubkey_decoded = device_pubkey.replace("\\n", "\n");
    trace!(
        "Retrieved JWT pubkey for device {}: {}",
        device_uuid, device_pubkey_decoded
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
        warn!(
            "Rejected device JWT due to too long lifetime: {} secs",
            difference_secs
        );
        return Err(jsonwebtoken::errors::ErrorKind::InvalidToken.into());
    }

    Ok(verified_token.claims)
}
