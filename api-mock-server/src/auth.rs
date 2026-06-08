use jsonwebtoken::{DecodingKey, Validation, decode, errors::ErrorKind};
use log::debug;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::JwtConfig;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claims {
    pub subject: String, // subject - user ID
    pub name: String,    // user display name
    pub expiry: usize,   // expiry timestamp (Unix time)
}

// helpers that map missing/invalid -> ErrorKind::InvalidToken
fn get_str(claim: &Value, key: &str) -> Result<String, jsonwebtoken::errors::Error> {
    claim
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.to_owned())
        .ok_or_else(|| jsonwebtoken::errors::Error::from(ErrorKind::InvalidToken))
}

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub fn validate_token(
    token: &str,
    config: &JwtConfig,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Value>(
        token,
        &DecodingKey::from_rsa_pem(config.public_key.as_bytes())?,
        &Validation::new(jsonwebtoken::Algorithm::RS512),
    )?;
    debug!("Extracted JWT data from request: {:?}", token_data);

    let payload = token_data.claims;

    let subject = get_str(&payload, &config.subject_claim)?;
    let name = get_str(&payload, &config.name_claim)?;

    let expiry = payload
        .get("exp")
        .and_then(Value::as_u64)
        .map(|n| n as usize)
        .ok_or_else(|| jsonwebtoken::errors::Error::from(ErrorKind::InvalidToken))?;

    Ok(Claims {
        subject,
        name,
        expiry,
    })
}
