use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tracing::error;

#[derive(Debug)]
pub enum AppError {
    NotFound,
    BadRequest(String),
    Internal(anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        match self {
            AppError::NotFound => StatusCode::NOT_FOUND.into_response(),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg).into_response(),
            AppError::Internal(e) => {
                error!("{}", e);
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound,
            _ => AppError::Internal(e.into()),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e)
    }
}

impl AppError {
    pub fn internal(e: impl Into<anyhow::Error>) -> Self {
        AppError::Internal(e.into())
    }

    pub fn from_s3(e: SdkError<GetObjectError>) -> Self {
        // SdkError's Display only ever renders a generic summary like
        // "service error" - the specific error code (e.g. NoSuchKey) is
        // only available through the typed service-error accessor.
        let not_found = e.as_service_error().is_some_and(|err| err.is_no_such_key());
        if not_found {
            AppError::NotFound
        } else {
            AppError::Internal(anyhow::anyhow!("s3 error: {e:?}"))
        }
    }
}
