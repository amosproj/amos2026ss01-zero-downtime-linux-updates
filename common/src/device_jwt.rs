use serde::{Deserialize, Serialize};

pub const MAX_TOKEN_LIFETIME: i64 = 300;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claims {
    pub sub: String,  // subject - device uuid
    pub exp: i64,     // expiry timestamp (Unix time)
    pub role: String, // always `device`
}
