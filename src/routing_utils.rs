use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum_macros::FromRequest;

use serde::Serialize;
use utoipa::openapi::{RefOr, Schema};
use utoipa::{openapi, ToResponse, ToSchema};

use validator::ValidationErrors;

use crate::db::DbError;

/// Contains diagnostic information about an API failure
#[derive(Serialize, Debug, ToResponse)]
#[response(examples(
    ("Not Found" = (
        summary = "Entity could not be found (404)",
        value = json!({
            "error_code": "not_found",
            "error_description": "The requested entity could not be found.",
            "extra_info": null
        })
    )),

    ("Internal Failure" = (
        summary = "Something unexpected went wrong inside the server (500)",
        value = json!({
            "error_code": "internal_error",
            "error_description": "Could not access data to complete your request",
            "extra_info": null
        })
    )),

    ("Invalid Input" = (
        summary = "Invalid request body was passed (400)",
        value = json!({
            "error_code": "invalid_input",
            "error_description": "Submitted data was invalid.",
            "extra_info": {
                "first_name": [
                    {
                        "code": "length",
                        "message": null,
                        "params": {
                            "value": "Nameeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                            "max": 30
                        }
                    }
                ]
            }
        })
    )),

    ("Malformed JSON" = (
        summary = "Invalid JSON passed to server (400)",
        value = json!({
            "error_code": "invalid_json",
            "error_description": "The passed request body contained malformed or unreadable JSON.",
            "extra_info": "Failed to parse the request body as JSON: EOF while parsing an object at line 4 column 0"
        })
    ))
))]
pub struct BasicErrorResponse {
    error_code: String,
    error_description: String,
    extra_info: Option<ExtraInfo>,
}

#[derive(Serialize, Debug, ToSchema)]
#[serde(untagged)]
pub enum ExtraInfo {
    ValidationIssues(ValidationErrorSchema),
    Message(String),
}

/// Stand-in OpenAPI schema for [ValidationErrors] which just provides an empty object
#[derive(Serialize, Debug)]
#[serde(transparent)]
pub struct ValidationErrorSchema(ValidationErrors);

impl<'schem> ToSchema<'schem> for ValidationErrorSchema {
    fn schema() -> (&'schem str, RefOr<Schema>) {
        (
            "ValidationErrorSchema",
            openapi::ObjectBuilder::new().into(),
        )
    }
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
