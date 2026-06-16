use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{TokenData, dangerous::insecure_decode};
use log::{debug, error};
use serde_json::Value;

use crate::{api_v1::db, auth_user::validate_user_token, config::JwtConfig};

pub async fn jwt_auth(
    State(jwt_config): State<JwtConfig>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Get the Authorization header
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    // 2. Make sure it starts with "Bearer "
    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header["Bearer ".len()..],
        _ => {
            // No token or wrong format — reject with 401
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // 3. Distinguish between device and user token
    //    A token is only assumed from a device if `"role": "device"` is contained inside the claim
    let token_data = match insecure_decode::<Value>(token) {
        Ok(data) => data,
        Err(_) => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let is_device = is_device_token(token_data);

    if is_device {
        // TODO
        return Err(StatusCode::UNAUTHORIZED);
    } else {
        // 4. Validate the token for a user
        match validate_user_token(token, &jwt_config) {
            Ok(claims) => {
                if let Err(err) = db::upsert_user(claims.clone()).await {
                    error!("Failed to upsert user into db: {:?}", err);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
    
                // 5. Attach the claims to the request so handlers can use them
                req.extensions_mut().insert(claims);
                // 6. Pass the request to the next layer
                return Ok(next.run(req).await);
            }
            Err(err) => {
                debug!("JWT rejected: {:?}", err);
                // Invalid or expired token — reject with 401
                return Err(StatusCode::UNAUTHORIZED);
            }
        }
    }
}

fn is_device_token(token: TokenData<Value>) -> bool {
    return token.claims.get("role")
        .and_then(|v| v.as_str())
        .map(|s| s == "device")
        .unwrap_or(false);
}
