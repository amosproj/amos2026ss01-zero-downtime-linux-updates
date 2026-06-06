use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use log::debug;

use crate::auth::validate_token;

pub async fn jwt_auth(
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
        Some(header) if header.starts_with("Bearer ") => {
            &header["Bearer ".len()..]
        }
        _ => {
            // No token or wrong format — reject with 401
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // 3. Validate the token
    match validate_token(token) {
        Ok(claims) => {
            // 4. Attach the claims to the request so handlers can use them
            req.extensions_mut().insert(claims);
            // 5. Pass the request to the next layer
            Ok(next.run(req).await)
        }
        Err(err) => {
            debug!("JWT rejected: {:?}", err);
            // Invalid or expired token — reject with 401
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
