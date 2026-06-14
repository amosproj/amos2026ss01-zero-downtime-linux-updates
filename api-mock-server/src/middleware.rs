use std::cell::RefCell;

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};
use log::{debug, error};
use sea_orm::ConnectionTrait;

use crate::api_v1::db;
use crate::audit_context::CURRENT_USER;
use crate::auth::validate_token;
use crate::config::JwtConfig;

async fn set_pg_session_user(subject: &str) -> Result<(), sea_orm::DbErr> {
    let db = crate::api_v1::db::db!();
    let sql = format!("SET app.audit_user = '{}'", subject.replace('\'', "''"));
    match db.execute_unprepared(&sql).await {
        Ok(_) => Ok(()),
        Err(err) => {
            let msg = format!("{:?}", err);
            if msg.contains("SQLite") || msg.contains("sqlite") || msg.contains("not supported") {
                debug!("Skipping PG session variable on non-Postgres backend");
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

pub async fn jwt_auth(
    State(jwt_config): State<JwtConfig>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());

    let token = match auth_header {
        Some(header) if header.starts_with("Bearer ") => &header["Bearer ".len()..],
        _ => {
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    match validate_token(token, &jwt_config) {
        Ok(claims) => {
            if let Err(err) = db::upsert_user(claims.clone()).await {
                error!("Failed to upsert user into db: {:?}", err);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            if let Err(err) = set_pg_session_user(&claims.subject).await {
                error!("Failed to set PG session user: {:?}", err);
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            let user_subject = claims.subject.clone();
            req.extensions_mut().insert(claims);

            let user_ref = RefCell::new(Some(user_subject));

            Ok(CURRENT_USER
                .scope(user_ref, async { next.run(req).await })
                .await)
        }
        Err(err) => {
            debug!("JWT rejected: {:?}", err);
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}
