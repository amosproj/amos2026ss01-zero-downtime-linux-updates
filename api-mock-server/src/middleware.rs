use std::cell::RefCell;

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use log::{debug, error};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend};

use crate::api_v1::db;
use crate::audit_context::CURRENT_USER;
use crate::auth::validate_token;
use crate::config::JwtConfig;

async fn set_pg_session_user(db: &DatabaseConnection, user_id: i32) -> Result<(), sea_orm::DbErr> {
    if db.get_database_backend() != DbBackend::Postgres {
        debug!("Skipping PG session variable on non-Postgres backend");
        return Ok(());
    }
    let sql = format!("SET app.audit_user = '{}'", user_id);
    db.execute_unprepared(&sql).await?;
    Ok(())
}

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

    // 3. Validate the token
    match validate_token(token, &jwt_config) {
        Ok(claims) => {
            // 4. Upsert user into the database
            let user = match db::upsert_user(claims.clone()).await {
                Ok(user) => user,
                Err(err) => {
                    error!("Failed to upsert user into db: {:?}", err);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            };

            // 5. Set PostgreSQL session variable for audit triggers.
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

            // 6. Attach the claims to the request so handlers can use them
            let user_subject = claims.subject.clone();
            req.extensions_mut().insert(claims);

            // 7. Set task-local user context and pass the request to the next layer
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
