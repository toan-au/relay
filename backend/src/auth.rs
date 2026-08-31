use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;

use crate::errors::AppError;
use crate::models::admin_session::AdminSession;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "admin_session";

/// Extractor that only succeeds if the request carries a valid admin
/// session cookie. Route handlers that take this as a parameter are
/// implicitly protected - axum rejects the request before the handler
/// body runs otherwise.
pub struct AdminAuth;

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(SESSION_COOKIE)
            .map(|c| c.value().to_string())
            .ok_or(AppError::Unauthorized)?;

        let valid = AdminSession::is_valid(&state.db, &token)
            .await
            .map_err(AppError::internal)?;

        if valid {
            Ok(AdminAuth)
        } else {
            Err(AppError::Unauthorized)
        }
    }
}
