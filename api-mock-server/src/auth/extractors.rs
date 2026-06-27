use axum::{extract::FromRequestParts, http::StatusCode};

/// An Extractor to match only authenticated devices.
/// Use as middleware with [`axum::middleware::from_extractor`].
/// Needs to be behind [`super::jwt_middleware`].
/// Contains the device UUID.
pub struct AuthDevice(pub String);

impl<S> FromRequestParts<S> for AuthDevice
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let claims = parts.extensions.get::<amos_common::device_jwt::Claims>();

        match claims {
            Some(c) => Ok(Self(c.sub.clone())),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

/// An Extractor to match only authenticated users.
/// Use as middleware with [`axum::middleware::from_extractor`].
/// Needs to be behind [`super::jwt_middleware`].
/// Contains the user's ID and name.
pub struct AuthUser(pub UserData);

pub struct UserData {
    id: String,
    name: String,
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let claims = parts.extensions.get::<super::user::Claims>();

        match claims {
            Some(c) => Ok(Self(UserData {
                id: c.subject.clone(),
                name: c.name.clone(),
            })),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}
