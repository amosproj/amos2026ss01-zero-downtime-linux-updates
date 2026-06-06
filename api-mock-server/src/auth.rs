use jsonwebtoken::{decode, DecodingKey, Validation};
use log::debug;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claims {
    pub sub: String,   // subject - user ID
    pub name: String,  // user display name
    pub exp: usize,    // expiry timestamp (Unix time)
}

// const SECRET: &[u8] = b"TODO";
// RSA: ssh-keygen -f test_jwt_key.pub -e -m pem
const SIGN_PUBKEY: &[u8] = b"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAD1UgygharWO7FJZ7koOmIwa4VFkniGHtOQjdd7mYBi8=
-----END PUBLIC KEY-----";

/// Validate a JWT string.
/// Returns the decoded Claims on success, or an error if the token is invalid/expired.
pub fn validate_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        // &DecodingKey::from_secret(SECRET),
        &DecodingKey::from_ed_pem(SIGN_PUBKEY)?,
        &Validation::new(jsonwebtoken::Algorithm::EdDSA),
    )?;
    debug!("Extracted JWT data: {:?}", token_data);

    Ok(token_data.claims)
}
