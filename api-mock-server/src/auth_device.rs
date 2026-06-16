use jsonwebtoken::{DecodingKey, TokenData, Validation, decode};
use serde_json::Value;

use amos_common::device_jwt::Claims;
use crate::auth_user::get_str;

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub fn validate_device_token(
    token: String,
    token_data: TokenData<Value>,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let device_uuid = get_str(&token_data.claims, "sub");

    let device_pubkey = ""; // TODO: Get pubkey from db

    let verified_token = decode::<Claims>(
        token,
        &DecodingKey::from_rsa_pem(device_pubkey.as_bytes())?,
        &Validation::new(jsonwebtoken::Algorithm::RS256),
    )?;

    Ok(verified_token.claims)
}
