use axum::{extract::FromRequestParts, http::StatusCode};

use crate::auth::device::ClientDevice;

/// An Extractor to match only authenticated devices.
/// Use as middleware with [`axum::middleware::from_extractor`].
/// Needs to be behind [`super::jwt_middleware`].
/// Contains the device UUID.
pub struct AuthDevice(pub ClientDevice);

impl<S> FromRequestParts<S> for AuthDevice
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _: &S,
    ) -> Result<Self, Self::Rejection> {
        let claims = parts.extensions.get::<super::device::ClientDevice>();

        match claims {
            Some(c) => Ok(Self(c.clone())),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

/// An Extractor to match only authenticated users.
/// Use as middleware with [`axum::middleware::from_extractor`].
/// Needs to be behind [`super::jwt_middleware`].
pub struct AuthUser;

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
            Some(_) => Ok(Self),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}
