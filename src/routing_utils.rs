use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_macros::FromRequest;


use serde::Serialize;

use validator::ValidationErrors;

use crate::db::DbError;

/// Contains diagnostic information about an API failure
#[derive(Serialize, Debug)]
pub struct BasicErrorResponse {
    error_code: String,
    error_description: String,
    extra_info: Option<ExtraInfo>,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum ExtraInfo {
    ValidationIssues(ValidationErrors),
    Message(String),
}

/// Response type that wraps database errors and turns them into [BasicErrorResponse]s
pub enum DbErrorResponse {
    NoResults,
    QueryFailure,
}

impl IntoResponse for DbErrorResponse {
    fn into_response(self) -> Response {
        match self {
            Self::NoResults => (
                StatusCode::NOT_FOUND,
                Json(BasicErrorResponse {
                    error_code: "not_found".into(),
                    error_description: "The requested entity could not be found.".into(),
                    extra_info: None,
                }),
            )
                .into_response(),

            Self::QueryFailure => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BasicErrorResponse {
                    error_code: "internal_error".into(),
                    error_description: "Could not access data to complete your request".into(),
                    extra_info: None,
                }),
            )
                .into_response(),
        }
    }
}

impl From<DbError> for DbErrorResponse {
    fn from(value: DbError) -> Self {
        match value {
            DbError::NoResults => Self::NoResults,
            DbError::QueryFailure(_) => Self::QueryFailure,
        }
    }
}

/// Response type that wraps validation errors and turns them into [BasicErrorResponse]s
pub struct ValidationErrorResponse(ValidationErrors);

impl IntoResponse for ValidationErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(BasicErrorResponse {
                error_code: "invalid_input".into(),
                error_description: "Submitted data was invalid.".to_owned(),
                extra_info: Some(ExtraInfo::ValidationIssues(self.0)),
            }),
        )
            .into_response()
    }
}

impl From<ValidationErrors> for ValidationErrorResponse {
    fn from(value: ValidationErrors) -> Self {
        Self(value)
    }
}

/// Wrapper for [axum::Json] which customizes the error response to use our
/// data structure for API errors
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(JsonErrorResponse))]
pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// Response type representing JSON parse errors
pub struct JsonErrorResponse {
    parse_problem: String,
}

impl From<JsonRejection> for JsonErrorResponse {
    fn from(value: JsonRejection) -> Self {
        JsonErrorResponse {
            parse_problem: value.body_text(),
        }
    }
}

impl IntoResponse for JsonErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            axum::Json(BasicErrorResponse {
                error_code: "invalid_json".into(),
                error_description:
                    "The passed request body contained malformed or unreadable JSON.".into(),
                extra_info: Some(ExtraInfo::Message(self.parse_problem)),
            }),
        )
            .into_response()
    }
}
