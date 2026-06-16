use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Claims {
    pub sub: String,  // subject - device uuid
    pub exp: usize,   // expiry timestamp (Unix time)
    pub role: String, // always `device`
}
