use std::fmt;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ErrorType {
    NotFound,
    Internal,
    UniqueViolation,
    Unauthorized,
    Validation,
}

#[derive(Debug)]
pub struct CustomError {
    pub err_type: ErrorType,
    pub message: String,
}

impl CustomError {
    pub fn new(message: impl Into<String>, err_type: ErrorType) -> Self {
        Self {
            message: message.into(),
            err_type,
        }
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::new(msg, ErrorType::NotFound)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(msg, ErrorType::Internal)
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new(msg, ErrorType::Unauthorized)
    }

    pub fn validation(msg: impl Into<String>) -> Self {
        Self::new(msg, ErrorType::Validation)
    }

    pub fn from_diesel_err(err: diesel::result::Error, context: &str) -> Self {
        let err_type = match &err {
            diesel::result::Error::DatabaseError(diesel::result::DatabaseErrorKind::UniqueViolation, _) => {
                ErrorType::UniqueViolation
            }
            diesel::result::Error::NotFound => ErrorType::NotFound,
            _ => ErrorType::Internal,
        };
        Self::new(format!("{context}: {err}"), err_type)
    }

    fn status(&self) -> StatusCode {
        match self.err_type {
            ErrorType::NotFound => StatusCode::NOT_FOUND,
            ErrorType::UniqueViolation | ErrorType::Validation => StatusCode::UNPROCESSABLE_ENTITY,
            ErrorType::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorType::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CustomError {}

impl IntoResponse for CustomError {
    fn into_response(self) -> Response {
        let status = self.status();
        // Don't leak internal error details to the client for 5xx.
        let body = if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self.message, "internal server error");
            json!({ "error": "Internal server error" })
        } else {
            json!({ "error": self.message })
        };
        (status, Json(body)).into_response()
    }
}

impl From<diesel::result::Error> for CustomError {
    fn from(err: diesel::result::Error) -> Self {
        Self::from_diesel_err(err, "database error")
    }
}
