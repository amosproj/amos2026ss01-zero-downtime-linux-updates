use jsonwebtoken::{DecodingKey, Validation, decode};
use log::debug;
use serde::{Deserialize, Serialize};

use crate::config::JwtConfig;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claims {
    pub sub: String,  // subject - user ID
    pub name: String, // user display name
    pub exp: usize,   // expiry timestamp (Unix time)
}

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub fn validate_token(
    token: &str,
    config: &JwtConfig,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_rsa_pem(config.public_key.as_bytes())?,
        &Validation::new(jsonwebtoken::Algorithm::RS512),
    )?;
    debug!("Extracted JWT data from request: {:?}", token_data);

    Ok(token_data.claims)
}
