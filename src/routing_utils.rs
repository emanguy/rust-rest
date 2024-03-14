use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_macros::FromRequest;

use serde::Serialize;

use crate::dto::{BasicError, ExtraInfo, ValidationErrorSchema};
use validator::ValidationErrors;

/// Represents a generic 500 internal server error which turns into a [BasicError]
pub struct GenericErrorResponse(pub anyhow::Error);

impl IntoResponse for GenericErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(BasicError {
                error_code: "internal_error".to_owned(),
                error_description: format!("An unexpected error occurred: {}", self.0),
                extra_info: None,
            }),
        )
            .into_response()
    }
}

/// Response type that wraps validation errors and turns them into [BasicError]s
pub struct ValidationErrorResponse(ValidationErrors);

impl IntoResponse for ValidationErrorResponse {
    fn into_response(self) -> Response {
        (
            StatusCode::BAD_REQUEST,
            Json(BasicError {
                error_code: "invalid_input".into(),
                error_description: "Submitted data was invalid.".to_owned(),
                extra_info: Some(ExtraInfo::ValidationIssues(ValidationErrorSchema(self.0))),
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
#[cfg_attr(test, derive(Debug))]
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
            axum::Json(BasicError {
                error_code: "invalid_json".into(),
                error_description:
                    "The passed request body contained malformed or unreadable JSON.".into(),
                extra_info: Some(ExtraInfo::Message(self.parse_problem)),
            }),
        )
            .into_response()
    }
}
