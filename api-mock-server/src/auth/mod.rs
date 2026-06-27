mod device;
pub mod extractors;
pub mod user;

use crate::api_v1::db;
use crate::audit_context::CURRENT_USER;
use crate::config::JwtConfig;
use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{TokenData, dangerous::insecure_decode, errors::ErrorKind};
use log::{debug, error, trace};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend};
use serde_json::Value;
use std::cell::RefCell;

pub async fn jwt_middleware(
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

    let is_device = is_device_token(&token_data);

    if is_device {
        // 4. Validate the token for a device
        trace!("Received device JWT: {}", token);
        match device::validate_token(token.to_owned(), token_data).await {
            Ok(claims) => {
                // 5. Attach the claims to the request so handlers can use them
                req.extensions_mut().insert(claims);
                // 6. Pass the request to the next layer
                Ok(next.run(req).await)
            }
            Err(err) => {
                debug!("JWT rejected: {:?}", err);
                // Invalid or expired token — reject with 401
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    } else {
        // 4. Validate the token for a user
        trace!("Received user JWT: {}", token);
        match user::validate_token(token, &jwt_config) {
            Ok(claims) => {
                // 5. Upsert user into the database
                let user = match db::upsert_user(claims.clone()).await {
                    Ok(user) => user,
                    Err(err) => {
                        error!("Failed to upsert user into db: {:?}", err);
                        return Err(StatusCode::INTERNAL_SERVER_ERROR);
                    }
                };
                // 6. Set PostgreSQL session variable for audit triggers.
                //    We use SET (not SET LOCAL) because the SET and the subsequent
                //    data-changing query run as separate db.execute() calls.
                //    SET LOCAL would be lost when the implicit transaction commits.
                //    The connection pool is configured with max_connections=1 (see
                //    initialialize_db) so all operations within a single request
                //    share the same connection, preventing user-context leakage
                //    across requests.
                let conn = db::DB.read().await.clone().unwrap();
                if let Err(err) = set_pg_session_user(&conn, user.id).await {
                    error!("Failed to set PG session user: {:?}", err);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
                // 7. Attach the claims to the request so handlers can use them
                let user_subject = claims.subject.clone();
                req.extensions_mut().insert(claims);
                // 8. Set task-local user context and pass the request to the next layer
                let user_ref = RefCell::new(Some(user_subject));
                Ok(CURRENT_USER
                    .scope(user_ref, async { next.run(req).await })
                    .await)
            }
            Err(err) => {
                debug!("JWT rejected: {:?}", err);
                // Invalid or expired token — reject with 401
                Err(StatusCode::UNAUTHORIZED)
            }
        }
    }
}

async fn set_pg_session_user(db: &DatabaseConnection, user_id: i32) -> Result<(), sea_orm::DbErr> {
    if db.get_database_backend() != DbBackend::Postgres {
        debug!("Skipping PG session variable on non-Postgres backend");
        return Ok(());
    }
    let sql = format!("SET app.audit_user = '{}'", user_id);
    db.execute_unprepared(&sql).await?;
    Ok(())
}

fn is_device_token(token: &TokenData<Value>) -> bool {
    token
        .claims
        .get("role")
        .and_then(|v| v.as_str())
        .map(|s| s == "device")
        .unwrap_or(false)
}

/// helpers that map missing/invalid -> ErrorKind::InvalidToken
fn extract_claim(claim: &Value, key: &str) -> Result<String, jsonwebtoken::errors::Error> {
    claim
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.to_owned())
        .ok_or_else(|| jsonwebtoken::errors::Error::from(ErrorKind::InvalidToken))
}
